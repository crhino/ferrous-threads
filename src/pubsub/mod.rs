use std::sync::mpsc::{channel, Sender, SendError, Receiver};
use std::error::Error;

pub struct Publisher<T> {
    senders: Vec<Sender<T>>
}

pub type Subscriber<T> = Receiver<T>;

impl<T: Clone> Publisher<T> {
    pub fn new() -> Publisher<T> {
        Publisher {
            senders: Vec::new(),
        }
    }

    pub fn subscribe(&mut self, sub: Sender<T>) {
        self.senders.push(sub);
    }

    pub fn publish(&mut self, sub: T) -> Result<(), SendError<T>> {
        for s in self.senders.iter() {
            let res = s.send(sub.clone());
            if res.is_err() {
                return res
            }
        }
        Ok(())
    }
}

pub fn pubsub_channel<T: Clone>(num_subs: usize) -> (Publisher<T>, Vec<Subscriber<T>>) {
        let mut p = Publisher::new();
        let mut vec = Vec::new();

        for _a in 0..num_subs {
            let (sender, sub) = channel();
            p.subscribe(sender);
            vec.push(sub);
        }
        (p, vec)
}

#[cfg(test)]
mod test {
    use super::{Publisher, Subscriber, pubsub_channel};
    use std::sync::mpsc::{channel};

    #[test]
    fn test_pubsub() {
        let (sn1, s1) = channel::<usize>();
        let (sn2, s2) = channel::<usize>();
        let mut p = Publisher::new();
        p.subscribe(sn1);
        p.subscribe(sn2);

        let res = p.publish(9);
        assert!(res.is_ok());

        let res = s1.recv();
        assert!(res.ok().unwrap() == 9);
        let res = s2.recv();
        assert!(res.ok().unwrap() == 9);
    }

    #[test]
    fn test_pubsub_channel() {
        let (mut p, subs) = pubsub_channel::<usize>(3);


        let res = p.publish(9);
        assert!(res.is_ok());
        assert!(subs.len() == 3);

        for s in subs.iter() {
            let res = s.recv();
            assert!(res.ok().unwrap() == 9);
        }
    }
}
