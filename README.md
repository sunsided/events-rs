# Event Handling

A .NET `event`, `EventHandler` and `EventArgs` inspired event handling system.

## Examples

The following example constructs an `Event` and two handles to it.
It invokes the event through one of the handles (de-registering it), then on the event itself.

```rust
use event_handler::prelude::*;
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
    let update_first_value = {
        let first_value = value.clone();
        move |amount| *first_value.lock().unwrap() = amount
    };

    // Create a closure to mutate the other value.
    let update_second_value = {
        let second_value = value2.clone();
        move |amount| *second_value.lock().unwrap() = amount * 2
    };

    // Register the function to the event.
    let handle = event.add_fn(update_first_value).unwrap();
    let _handle = event.add_fn(update_second_value).unwrap();

    // Two handlers are now registered.
    assert_eq!(event.len(), 2);

    // Invoke the event on the handle.
    // Since we move the handle, this will de-register the handler when the scope is left.
    assert!(std::thread::spawn(move || { handle.invoke(41) })
        .join()
        .is_ok());
    assert_eq!(event.len(), 1);
    assert_eq!(*value.lock().unwrap(), 41);
    assert_eq!(*value2.lock().unwrap(), 41 * 2);

    // Invoke the event on the event itself.
    event.invoke(42);
    assert_eq!(*value.lock().unwrap(), 41);         // previous value
    assert_eq!(*value2.lock().unwrap(), 42 * 2);    // updated value
}
```

If the `Event` is dropped, calls to `EventHandle::invoke` will return an error.

```rust
use event_handler::prelude::*;
use std::sync::{Arc, Mutex};

#[test]
fn it_works() {
    let event = Event::new();

    // Register the function to the event.
    let value = Arc::new(Mutex::new(0));
    let handle = event.add_fn({
        let value = value.clone();
        move |amount| *value.lock().unwrap() = amount
    }).unwrap();

    // Register another function.
    let value2 = Arc::new(Mutex::new(0));
    let late_handle = event.add_fn({
        let value = value2.clone();
        move |amount| *value.lock().unwrap() = amount * 2
    }).unwrap();

    // Invoke the event on the handler itself.
    // This will move the event, dropping it afterwards.
    assert!(std::thread::spawn(move || {
        event.invoke(42);
    })
        .join()
        .is_ok());
    assert_eq!(*value.lock().unwrap(), 42);
    assert_eq!(*value2.lock().unwrap(), 42 * 2);

    // This event invocation will fail because the event is already dropped.
    assert_eq!(
        std::thread::spawn(move || { late_handle.invoke(41) })
            .join()
            .unwrap(),
        Err(EventInvocationError::EventDropped)
    );
}
```
