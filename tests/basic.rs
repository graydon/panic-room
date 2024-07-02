use panic_room::{Room, Handle};

thread_local! {
    static DROP_COUNT: std::cell::RefCell<usize> = std::cell::RefCell::new(0);
}

struct HasDrop;

impl Drop for HasDrop {
    fn drop(&mut self) {
        DROP_COUNT.with(|c| *c.borrow_mut() += 1);
    }
}

impl HasDrop {
    fn hello(&self) {
        println!("hello");
    }
}

fn panics() {
    panic!("oh no!");
}

fn uses_a_handle(handle: Handle<HasDrop>) {
    Room::with_current(|room| {
        room?.get(handle).map(|r| {
            r.hello();
        });
        Some(())
    });
}

fn calls_a_panic() -> Option<()> {
    let handle = 
    Room::with_current_mut(|room| {
        Some(room?.alloc(HasDrop))
    })?;
    uses_a_handle(handle);
    panics();
    Some(())
}

#[test]
fn test_contain_panics() {
    let res = Room::contain_panics(|| {
        calls_a_panic();
    });
    println!("recovered"); 
    dbg!(res);
    dbg!(DROP_COUNT.with(|c| *c.borrow()));   
    assert!(res.is_none());
    assert_eq!(DROP_COUNT.with(|c| *c.borrow()), 1);
}