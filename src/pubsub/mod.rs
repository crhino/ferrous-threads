use std::sync::mpsc::{channel, Sender, SendError, Receiver, RecvError};
use std::error::Error;
use std::sync::{Arc, RwLock};
use std::fmt;
use std::any::Any;
use std::convert::From;

pub struct Subscriber<T> {
    receiver: Receiver<T>,
    self_sender: Sender<T>,
    publishers: Vec<Arc<Publisher<T>>>,
}

impl<T: Clone> Subscriber<T> {
    pub fn new() -> Subscriber<T>  {
        let (sn, rc) = channel();
        Subscriber {
            receiver: rc,
            self_sender: sn,
            publishers: Vec::new(),
        }
    }

    pub fn subscribe(&mut self, p: &Arc<Publisher<T>>) {
        self.publishers.push(p.clone());
        p.subscribe(self.self_sender.clone());
    }

    pub fn recv(&self) -> Result<T, PubSubError<T>> {
        let data = try!(self.receiver.recv());
        Ok(data)
    }
}

impl<T: Clone> Clone for Subscriber<T> {
    fn clone(&self) -> Subscriber<T> {
        let sub = Subscriber::new();
        for p in self.publishers.iter() {
            p.subscribe(sub.self_sender.clone());
        }
        sub
    }
}

pub struct Publisher<T> {
    senders: RwLock<Vec<Sender<T>>>
}

impl<T: Clone> Publisher<T> {
    pub fn new() -> Arc<Publisher<T>> {
        Arc::new(Publisher {
            senders: RwLock::new(Vec::new()),
        })
    }

    fn subscribe(&self, sub: Sender<T>) {
        let mut vec = self.senders.write().unwrap();
        vec.push(sub);
    }

    pub fn publish(&self, sub: T) -> Result<(), PubSubError<T>> {
        let vec = self.senders.read().unwrap();
        for s in vec.iter() {
            try!(s.send(sub.clone()));
        }
        Ok(())
    }
}


#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct PubSubError<T> {
    kind: ErrorKind<T>,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum ErrorKind<T> {
    SendError(SendError<T>),
    RecvError(RecvError),
}

impl<T> From<RecvError> for PubSubError<T> {
    fn from(err: RecvError) -> PubSubError<T> {
        PubSubError {
            kind: ErrorKind::RecvError(err),
        }
    }
}

impl<T> From<SendError<T>> for PubSubError<T> {
    fn from(err: SendError<T>) -> PubSubError<T> {
        PubSubError {
            kind: ErrorKind::SendError(err),
        }
    }
}

impl<T> fmt::Display for PubSubError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.kind {
            ErrorKind::SendError(ref se) => se.fmt(f),
            ErrorKind::RecvError(ref re) => re.fmt(f),
        }
    }
}

impl<T: Any + fmt::Debug + Send> Error for PubSubError<T> {
    fn description(&self) -> &str {
        match self.kind {
            ErrorKind::SendError(ref se) => se.description(),
            ErrorKind::RecvError(ref re) => re.description(),
        }
    }

    fn cause(&self) -> Option<&Error> {
        match self.kind {
            ErrorKind::SendError(ref se) => se.cause(),
            ErrorKind::RecvError(ref re) => re.cause(),
        }
    }
}

pub fn pubsub_channel<T: Clone>() -> (Arc<Publisher<T>>, Subscriber<T>) {
        let p = Publisher::new();
        let mut s = Subscriber::new();
        s.subscribe(&p);
        (p, s)
}

#[cfg(test)]
mod test {
    use super::{Publisher, Subscriber, pubsub_channel};

    #[test]
    fn test_pubsub() {
        let mut s1 = Subscriber::new();
        let mut s2 = Subscriber::new();
        let p = Publisher::new();
        s1.subscribe(&p);
        s2.subscribe(&p);

        let res = p.publish(9);
        assert!(res.is_ok());

        let res = s1.recv();
        assert!(res.ok().unwrap() == 9);
        let res = s2.recv();
        assert!(res.ok().unwrap() == 9);
    }

    #[test]
    fn test_sub_clone() {
        let p = Publisher::new();

        let mut sub1 = Subscriber::new();
        sub1.subscribe(&p);
        let sub2 = sub1.clone();

        let res = p.publish(9);
        assert!(res.is_ok());

        let res = sub2.recv();
        assert!(res.ok().unwrap() == 9);
        let res = sub1.recv();
        assert!(res.ok().unwrap() == 9);
    }

    #[test]
    fn test_sub_clone_multi_publish() {
        let p = Publisher::new();
        let p2 = Publisher::new();

        let mut sub1 = Subscriber::new();
        sub1.subscribe(&p);
        sub1.subscribe(&p2);
        let sub2 = sub1.clone();

        let res = p.publish(9);
        assert!(res.is_ok());
        let res = p2.publish(3);
        assert!(res.is_ok());

        let res = sub2.recv();
        assert!(res.ok().unwrap() == 9);
        let res = sub1.recv();
        assert!(res.ok().unwrap() == 9);
        let res = sub1.recv();
        assert!(res.ok().unwrap() == 3);
        let res = sub2.recv();
        assert!(res.ok().unwrap() == 3);
    }

    #[test]
    fn test_pubsub_channel() {
        let (p, sub) = pubsub_channel::<usize>();


        let res = p.publish(9);
        assert!(res.is_ok());

        let res = sub.recv();
        assert!(res.ok().unwrap() == 9);
    }
}
