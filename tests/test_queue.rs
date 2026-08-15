use twitch_tts::domain::models::{MessageStatus, SpokenItem};
use twitch_tts::domain::queue::OverflowQueue;

#[test]
fn test_overflow_queue_fifo_and_drop_oldest() {
    let queue = OverflowQueue::new(3);

    // Push 3 items
    let item1 = SpokenItem::new("user1".into(), "msg1".into(), "msg1".into(), MessageStatus::Queued);
    let item2 = SpokenItem::new("user2".into(), "msg2".into(), "msg2".into(), MessageStatus::Queued);
    let item3 = SpokenItem::new("user3".into(), "msg3".into(), "msg3".into(), MessageStatus::Queued);

    assert!(queue.push(item1).is_none());
    assert!(queue.push(item2).is_none());
    assert!(queue.push(item3).is_none());
    assert_eq!(queue.len(), 3);

    // Push 4th item -> should drop oldest (item1)
    let item4 = SpokenItem::new("user4".into(), "msg4".into(), "msg4".into(), MessageStatus::Queued);
    let dropped = queue.push(item4);
    assert!(dropped.is_some());
    let dropped_item = dropped.unwrap();
    assert_eq!(dropped_item.sender, "user1");
    assert_eq!(dropped_item.status, MessageStatus::DroppedOverflow);

    // Next item popped should be item2
    let popped = queue.pop().expect("Expected item2");
    assert_eq!(popped.sender, "user2");
    assert_eq!(queue.len(), 2);
}

#[test]
fn test_queue_clear() {
    let queue = OverflowQueue::new(5);
    for i in 0..4 {
        let item = SpokenItem::new(format!("user{}", i), "test".into(), "test".into(), MessageStatus::Queued);
        queue.push(item);
    }
    assert_eq!(queue.len(), 4);

    let cleared = queue.clear();
    assert_eq!(cleared.len(), 4);
    assert_eq!(queue.len(), 0);
    for item in cleared {
        assert_eq!(item.status, MessageStatus::DroppedOverflow);
    }
}
