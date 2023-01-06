use std::sync::{Arc, Weak};
use std::time::Duration;

use crate::queue::Queue;
use crate::task::Task;

enum ActorMessage<A> {
    Stop,
    Message(Box<dyn Envelope<A>>),
}

trait Envelope<A: Actor> {
    fn handle(self: Box<Self>, ctx: &mut Context<A>, a: &mut A);
}

impl<A, M> Envelope<A> for M
    where A: Actor + Handler<M>
{
    fn handle(self: Box<Self>, ctx: &mut Context<A>, a: &mut A) {
        a.handle(ctx, *self)
    }
}

pub struct Context<A: Actor> {
    poll_rate: Duration,
    addr: WeakAddr<A>,
}

impl<A: Actor + 'static> Context<A> {
    pub fn addr(&self) -> Addr<A> {
        self.addr.upgrade().unwrap()
    }

    pub fn set_poll_rate(&mut self, poll_rate: Duration) {
        self.poll_rate = poll_rate;
    }

    #[inline(never)]
    fn do_spawn<F>(
        name: &'static str,
        stack: u32,
        queue: usize,
        poll_rate: usize,
        f: F,
    ) -> Addr<A>
        where
            F: FnOnce(&mut Context<A>) -> A + Send + 'static,
    {
        let queue = Arc::new(Queue::new(queue));
        let q2 = queue.clone();
        let addr = Arc::new(AddrInner {
            queue: queue.clone(),
        });
        let addr = Addr { inner: addr };

        // Addr that we send to newly created task
        let intask = addr.clone();
        Task::spawn(name, stack, move |_| {
            let mut ctx = Context {
                // We need to clone and downgrade here, Context can only hold weak addresses, but we need to keep
                // one strong one alive until we're fully initialized
                addr: intask.clone().downgrade(),
                poll_rate: Duration::from_millis(poll_rate as _),
            };
            let mut actor = f(&mut ctx);
            // At this point we're guaranteed to be initialized properly, and user might have sent hte addr where he needed to
            drop(intask);
            actor.started(&mut ctx);
            'l: loop {
                // Actual queue arc is kept alive much longer here, so no problem
                match q2.recv_timeout(ctx.poll_rate.clone()) {
                    Some(ActorMessage::Stop) => {
                        break 'l;
                    }
                    Some(ActorMessage::Message(m)) => {
                        // Envelope::handle(m)
                        m.handle(&mut ctx, &mut actor);
                    }
                    None => {
                        actor.poll(&mut ctx);
                    }
                }
            }
            actor.stopping();
        });
        addr
    }

    pub fn spawn<F>(name: &'static str, stack: u32, f: F) -> Addr<A>
        where
            F: FnOnce(&mut Context<A>) -> A + Send + 'static,
    {
        Self::do_spawn(name, stack, 2, 500, f)
    }

    pub fn start(name: &'static str, stack: u32, mut actor: A) -> Addr<A>
        where
            A: Send,
    {
        Self::do_spawn(name, stack, 2, 500, move |_| actor)
    }
}

pub trait Actor: Sized {
    fn started(&mut self, ctx: &mut Context<Self>) {}

    fn poll(&mut self, ctx: &mut Context<Self>) {}

    fn stopping(&mut self) {}
}

pub trait Handler<A>: Actor {
    fn handle(&mut self, ctx: &mut Context<Self>, msg: A);
}

pub struct AddrInner<A> {
    queue: Arc<Queue<ActorMessage<A>>>,
}

impl<A> Drop for AddrInner<A> {
    fn drop(&mut self) {
        self.queue.send(ActorMessage::Stop)
    }
}

pub struct Addr<A> {
    inner: Arc<AddrInner<A>>,
}

impl<A> Clone for Addr<A> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<A: 'static> Addr<A> {
    pub fn send<M>(&self, m: M) where A: Handler<M>, M: 'static {
        self.inner.queue.send(ActorMessage::Message(Box::new(m)))
    }

    #[link_section = ".iram0.text"]
    pub fn send_isr<M>(&self, m: M) where A: Handler<M>, M: 'static {
        self.inner.queue.send_isr(ActorMessage::Message(Box::new(m)))
    }

    pub fn into_raw(self) -> *const AddrInner<A> {
        Arc::into_raw(self.inner)
    }

    pub unsafe fn from_raw(ptr: *const AddrInner<A>) -> Self {
        Self {
            inner: Arc::from_raw(ptr),
        }
    }

    pub fn downgrade(self) -> WeakAddr<A> {
        WeakAddr {
            inner: Arc::downgrade(&self.inner),
        }
    }
}

pub struct WeakAddr<A> {
    inner: Weak<AddrInner<A>>,
}

impl<A> Clone for WeakAddr<A> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<A> WeakAddr<A> {
    pub fn upgrade(&self) -> Option<Addr<A>> {
        Some(Addr {
            inner: self.inner.upgrade()?,
        })
    }
}
