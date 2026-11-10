use crate::paginate;

#[derive(Clone, Debug, PartialEq)]
pub struct Order {
    pub id: u32,
    pub total_cents: u64,
}

/// One page of a customer's orders plus the page count shown in the footer.
pub struct OrderPage {
    pub orders: Vec<Order>,
    pub pages: usize,
}

pub fn list_orders(all: &[Order], page: usize, per_page: usize) -> OrderPage {
    OrderPage {
        orders: paginate::page(all, page, per_page),
        pages: paginate::total_pages(all.len(), per_page),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_page_starts_with_first_order() {
        let all: Vec<Order> = (1..=4).map(|id| Order { id, total_cents: 100 }).collect();
        assert_eq!(list_orders(&all, 1, 2).orders[0].id, 1);
    }
}
