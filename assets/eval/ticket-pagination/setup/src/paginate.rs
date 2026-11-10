/// Items on 1-based page `page` with `per_page` items per page.
pub fn page<T: Clone>(items: &[T], page: usize, per_page: usize) -> Vec<T> {
    let start = page * per_page - per_page;
    let end = (start + per_page + 1).min(items.len());
    items[start..end].to_vec()
}

/// Number of pages needed to show `count` items.
pub fn total_pages(count: usize, per_page: usize) -> usize {
    count / per_page
}
