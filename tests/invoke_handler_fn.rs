use std::cell::Cell;
use std::sync::Arc;
use event_handler::EventHandler;

#[test]
fn it_works<'i>() {
    let x = Box::new(0);
    let y = Box::new(0);
    let addr = *x as *const usize;
    let _addr2 = *x as *const usize;
    let _addr3 = *y as *const usize;
    println!("{:?}", addr);

    let mut handler = EventHandler::new();
    let value = Arc::new(Cell::new(0));

    let value2 = value.clone();
    let _handle = handler
        .add_fn(move || do_something(value2.clone()))
        .unwrap();

    handler.invoke();

    assert_eq!(handler.len(), 1);
    assert_eq!(value.get(), 42);
}

fn do_something(ptr: Arc<Cell<i32>>) {
    ptr.set(42);
}
