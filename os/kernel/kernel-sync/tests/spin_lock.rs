use kernel_sync::SpinLock;
use std::{panic, thread};

#[test]
fn basic_lock_and_raii() {
    let l = SpinLock::new(0_u32);

    // take the lock, mutate, and drop
    {
        let mut g = l.lock();
        *g = 41;
    }

    // lock again; previous drop must have unlocked
    {
        let mut g = l.lock();
        *g += 1;
        assert_eq!(*g, 42);
    }
}

#[test]
fn try_lock_semantics() {
    let l = SpinLock::new(1u8);

    // first try_lock should succeed
    let g1 = l.try_lock();
    assert!(g1.is_some());
    assert_eq!(**g1.as_ref().unwrap(), 1);

    // while held, try_lock must fail
    let g2 = l.try_lock();
    assert!(g2.is_none());

    // dropping guard allows another try_lock
    drop(g1);
    let g3 = l.try_lock();
    assert!(g3.is_some());
}

#[test]
fn with_lock_works_and_unlocks() {
    let l = SpinLock::new(String::from("a"));
    let len = l.with_lock(|s| {
        s.push('b');
        s.len()
    });
    assert_eq!(len, 2);

    // lock must be free now
    let got = l.with_lock(|s| s.clone());
    assert_eq!(got, "ab");
}

#[test]
fn get_mut_allows_direct_mutation() {
    let mut l = SpinLock::new(vec![1, 2, 3]);
    // &mut self guarantees no contention; we should get a plain &mut T
    l.get_mut().push(4);
    assert_eq!(l.lock().as_slice(), &[1, 2, 3, 4]);
}

#[test]
fn contended_increments_are_exact_and_exclusive() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};
    use std::thread;

    let threads = 8; // keep small for determinism
    let iters = 5_000; // likewise

    let lock = Arc::new(SpinLock::new(0usize));
    let in_cs = Arc::new(AtomicUsize::new(0));
    let start = Arc::new(Barrier::new(threads));

    let mut handles = Vec::with_capacity(threads);
    for _ in 0..threads {
        let lock = Arc::clone(&lock);
        let in_cs = Arc::clone(&in_cs);
        let start = Arc::clone(&start);
        handles.push(thread::spawn(move || {
            start.wait();
            for _ in 0..iters {
                lock.with_lock(|v| {
                    let prev = in_cs.fetch_add(1, Ordering::SeqCst);
                    assert_eq!(prev, 0, "mutual exclusion violated");
                    *v += 1;
                    in_cs.fetch_sub(1, Ordering::SeqCst);
                });

                // yield only AFTER releasing the lock to reduce convoy effects
                thread::yield_now();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let total = lock.with_lock(|v| *v);
    assert_eq!(total, threads * iters);
    assert_eq!(in_cs.load(Ordering::SeqCst), 0);
}

#[test]
fn lock_is_released_on_panic() {
    let l = SpinLock::new(0u32);

    let res = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        l.with_lock(|v| {
            *v = 123;
            panic!("boom");
        });
    }));
    assert!(res.is_err(), "expected panic");

    // We should be able to lock again right away.
    let val = l.with_lock(|v| *v);
    assert_eq!(val, 123);
}

/// SpinLock<T> is Sync when T: Send
fn _assert_sync_when_t_send<T: Send>() {
    fn assert_sync<S: Sync>(_s: &S) {}
    let l: SpinLock<T> =
        SpinLock::new(unsafe { core::mem::MaybeUninit::<T>::uninit().assume_init() });
    // We never run this; it only needs to type-check.
    let _ = || assert_sync(&l);
}

/// Spot-check a concrete instantiation compiles as Sync.
#[test]
fn spinlock_is_sync_for_send_t() {
    // If this compiles, SpinLock<u8> is Sync.
    fn takes_sync<S: Sync>(_s: &S) {}
    let l = SpinLock::new(0u8);
    takes_sync(&l);
}
