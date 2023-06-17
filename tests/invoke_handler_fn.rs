use event_handler::{Event, EventInvocationError};
use std::sync::{Arc, Mutex};

#[test]
fn it_works() {
    // The values we want to mutate.
    // These need to be Send such that the handle functions can update them.
    let value = Arc::new(Mutex::new(0));
    let value2 = Arc::new(Mutex::new(0));

    // Create an event handler.
    let event = Event::new();

    // Create a closure to mutate the value.
    let first_value = value.clone();
    let update_first_value = move |amount| *first_value.lock().unwrap() = amount;

    // Do the same thing again, but later.
    let first_value = value.clone();
    let update_first_value_again = move |amount| *first_value.lock().unwrap() = amount;

    // Create a closure to mutate the value.
    let second_value = value2.clone();
    let update_second_value = move |amount| *second_value.lock().unwrap() = amount * 2;

    // Register the function to the event.
    let handle = event.add_fn(update_first_value).unwrap();
    let _handle = event.add_fn(update_second_value).unwrap();
    let late_handle = event.add_fn(update_first_value_again).unwrap();

    // Three handlers are now registered.
    assert_eq!(event.len(), 3);

    // Invoke the event on the handle.
    assert!(std::thread::spawn(move || { handle.invoke(41) })
        .join()
        .is_ok());
    assert_eq!(*value.lock().unwrap(), 41);
    assert_eq!(*value2.lock().unwrap(), 41 * 2);

    // Invoke the event on the handler itself.
    // This will consume the handler, dropping the event.
    assert!(std::thread::spawn(move || {
        event.invoke(42);
    })
    .join()
    .is_ok());
    assert_eq!(*value.lock().unwrap(), 42);
    assert_eq!(*value2.lock().unwrap(), 42 * 2);

    // Invoke the event on the handle.
    // This call fails because the event is now unregistered.
    assert_eq!(
        std::thread::spawn(move || { late_handle.invoke(41) })
            .join()
            .unwrap(),
        Err(EventInvocationError::EventDropped)
    );
}
