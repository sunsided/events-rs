use event_handler::EventHandler;
use std::cell::Cell;
use std::sync::Arc;

#[test]
fn it_works() {
    // The values we want to mutate.
    let value = Arc::new(Cell::new(0));
    let value2 = Arc::new(Cell::new(0));

    // Create an event handler.
    let mut handler = EventHandler::new();

    // Create a closure to mutate the value.
    let do_something_value = value.clone();
    let do_something = move |amount| do_something_value.set(amount);

    // Create a closure to mutate the value.
    let do_something_value = value2.clone();
    let do_something_else = move |amount| do_something_value.set(amount * 2);

    // Register the function to the event.
    let handle = handler.add_fn(do_something).unwrap();
    let _handle = handler.add_fn(do_something_else).unwrap();

    // Two handlers are now registered.
    assert_eq!(handler.len(), 2);

    // Invoke the event on the handler itself.
    handler.invoke(42);
    assert_eq!(value.get(), 42);
    assert_eq!(value2.get(), 42 * 2);

    // Invoke the event on the handle.
    handle.invoke(41).ok();
    assert_eq!(value.get(), 41);
    assert_eq!(value2.get(), 41 * 2);
}
