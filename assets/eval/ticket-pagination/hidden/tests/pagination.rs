use shop::orders::{list_orders, Order};
use shop::paginate::{page, total_pages};

fn orders(n: u32) -> Vec<Order> {
    (1..=n).map(|id| Order { id, total_cents: 100 }).collect()
}

fn ids(all: &[Order], p: usize, per: usize) -> Vec<u32> {
    list_orders(all, p, per).orders.iter().map(|o| o.id).collect()
}

#[test]
fn pages_do_not_overlap() {
    let all = orders(5);
    assert_eq!(ids(&all, 1, 2), vec![1, 2]);
    assert_eq!(ids(&all, 2, 2), vec![3, 4]);
    assert_eq!(ids(&all, 3, 2), vec![5]);
}

#[test]
fn footer_counts_the_partial_last_page() {
    assert_eq!(list_orders(&orders(5), 1, 2).pages, 3);
    assert_eq!(total_pages(4, 2), 2);
}

#[test]
fn page_past_the_end_is_empty_not_a_panic() {
    assert!(ids(&orders(5), 9, 2).is_empty());
    assert!(page(&[1, 2, 3], 4, 3).is_empty());
}
