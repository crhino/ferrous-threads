extern crate ferrous_threads;
extern crate bit_vec;

use ferrous_threads::task_runner::TaskRunner;
use std::sync::mpsc::{Sender, Receiver, channel};
use bit_vec::BitVec;
use std::env;


// http://home.math.au.dk/himsen/Project1_parallel_algorithms_Torben.pdf

fn main() {
    let mut args = env::args();
    if args.len() != 2 {
        println!("Must specify either 'parallel' or 'sequential'");
        return
    }

    let arg = args.nth(1).unwrap();
    if arg == String::from("parallel") {
        parallel_sieve(100_000_000, 10);
    } else if arg == String::from("sequential") {

        let vec = sequential_sieve(100_000_000);
        println!("# of Primes: {}", vec.len());
    } else {
        println!("Must specify either 'parallel' or 'sequential'");
    }
}

fn sequential_sieve(n: usize) -> Vec<usize> {
    let mut n_array = BitVec::from_elem(n+1, true);
    let mut primes = Vec::new();
    let sqrt_n = (n as f64).sqrt() as usize + 1;

    for i in 2..sqrt_n {
        if !n_array.get(i).unwrap() {
            continue
        }

        primes.push(i);

        // We can start from i^2 because every composite smaller is already marked.
        let mut p = i * i;
        while p <= n {
            n_array.set(p, false);
            p = p + i;
        }
    }

    for i in sqrt_n..n {
        if n_array.get(i).unwrap() {
            primes.push(i);
        }
    }

    primes
}

fn parallel_sieve(n: usize, num_threads: u8) {
    let pool = TaskRunner::new(num_threads);
    let (sn_primes, accum_primes) = channel::<Option<usize>>();

    let sqrt_n = (n as f64).sqrt() as usize + 1;

    // Ensure that the first worker has indexes from o..sqrt_n
    // Try and avoid overflow in max_workers by casting num_threads to usize instead.
    let max_workers = n / (sqrt_n + 1);
    let num_workers = if (num_threads as usize) < max_workers { num_threads as usize } else { max_workers };
    let num_workers = num_workers as u8;

    let mut sn_sieves = Vec::new();
    let mut rc_sieves = Vec::new();
    for _i in 1..num_workers {
        let (sn, rc) = channel::<Option<usize>>();
        sn_sieves.push(sn);
        rc_sieves.push(rc);
    }

    let sn_p = sn_primes.clone();
    pool.enqueue(move || { sieve_master(n, num_workers, sn_sieves, sn_p) }).ok().expect("Could not enqueue task.");

    for (i, rc) in rc_sieves.into_iter().enumerate()  {
        let id = i as u8 + 1;
        let sn_p = sn_primes.clone();
        pool.enqueue(move || { sieve_worker(n, num_workers, id, rc, sn_p) }).ok().expect("Could not enqueue task.");
    }
    pool.enqueue(move || { prime_collector(num_workers, accum_primes) }).ok().expect("Could not enqueue task.");
}

fn sieve_master(n: usize, n_threads: u8, sn_sieves: Vec<Sender<Option<usize>>>, sn_primes: Sender<Option<usize>>) {
    let id = 0;
    let (_l, right) = bit_vec_endpoints(n, n_threads, id);
    // Start from 2.
    let left = 2;
    let mut sieve_vec = BitVec::from_elem(right - left + 1, true);
    let len = sieve_vec.len();
    for i in 0..len {
        if sieve_vec.get(i).unwrap() {
            let prime = left + i;
            send_prime_to_workers(prime, &sn_sieves);
            sieve_vector(&mut sieve_vec, id, prime, left, right);
        }
    }

    stop_workers(&sn_sieves);
    send_primes_to_collector(&mut sieve_vec, left, &sn_primes)
}

fn send_primes_to_collector(vec: &BitVec, left: usize, sn_primes: &Sender<Option<usize>>) {
    for (i, prime) in vec.iter().enumerate() {
        if prime {
            let prime = left + i;
            assert!(sn_primes.send(Some(prime)).is_ok());
        }
    }
    assert!(sn_primes.send(None).is_ok());
}

fn stop_workers(sn_sieves: &Vec<Sender<Option<usize>>>) {
    for sn in sn_sieves.iter() {
        assert!(sn.send(None).is_ok());
    }
}

fn send_prime_to_workers(p: usize, sn_sieves: &Vec<Sender<Option<usize>>>) {
    for sn in sn_sieves.iter() {
        assert!(sn.send(Some(p)).is_ok());
    }
}

// Each worker holds indices from (id * (n / n_threads)) to ((id + 1) * (n / n_threads) - 1)
// This will change when/if we use the easy reduction of only considering odd numbers.
fn sieve_worker(n: usize, n_threads: u8, id: u8, rc_sieve: Receiver<Option<usize>>, sn_primes: Sender<Option<usize>>) {
    let (left, right) = bit_vec_endpoints(n, n_threads, id);
    let mut sieve_vec = BitVec::from_elem(right - left + 1, true);

    loop {
        let sieve_num = match rc_sieve.recv() {
            Ok(Some(num)) => num,
            Ok(None) => break,
            Err(ref e)  => panic!("{}", e),
        };

        sieve_vector(&mut sieve_vec, id, sieve_num, left, right);
    }

    send_primes_to_collector(&mut sieve_vec, left, &sn_primes)
}

fn sieve_vector(vec: &mut BitVec, id: u8, sieve_num: usize, left: usize, right: usize) {
    let offset = calc_prime_offset(sieve_num, left, right);
    if offset == -1 {
        return
    }

    let mut offset = offset as usize;

    if id == 0 {
        offset = offset + sieve_num;
    }

    let len = vec.len();
    while offset < len {
        vec.set(offset, false);
        offset = offset + sieve_num;
    }
}

fn bit_vec_endpoints(n: usize, n_threads: u8, id: u8) -> (usize, usize) {
    let i = id as usize;
    let t = n_threads as usize;
    let left_endpt = (i * n) / t;
    let right_endpt = (((i + 1) * n) / t) - 1;
    (left_endpt, right_endpt)
}

fn calc_prime_offset(prime: usize, left: usize, right: usize) -> isize {
    if left % prime == 0 {
        return 0
    }

    let res = prime - (left % prime);
    if res + left > right {
        return -1
    }

    res as isize
}

fn prime_collector(num_thrs: u8, rc_primes: Receiver<Option<usize>>) {
    let mut primes = Vec::new();
    let mut num_done = 0;

    loop {
        if num_done == num_thrs {
            break
        }

        match rc_primes.recv() {
            Ok(Some(p)) => primes.push(p),
            Ok(None) => {
                num_done = num_done + 1;
            },
            Err(ref e) => panic!("{:?}", e),
        }
    }
    println!("# of Primes: {}", primes.len());
}
