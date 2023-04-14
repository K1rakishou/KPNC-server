use std::cmp::Ordering;

use crate::model::data::chan::PostDescriptor;

pub fn compare_post_descriptors(this: &PostDescriptor, other: &PostDescriptor) -> Ordering {
    let site_name_ordering = this.site_name().partial_cmp(other.site_name()).unwrap_or(Ordering::Less);
    if site_name_ordering != Ordering::Equal {
        return site_name_ordering;
    }

    let board_code_ordering = (*this.board_code()).partial_cmp(other.board_code()).unwrap_or(Ordering::Less);
    if board_code_ordering != Ordering::Equal {
        return board_code_ordering;
    }

    if this.thread_no() < other.thread_no() {
        return Ordering::Less;
    } else if this.thread_no() > other.thread_no() {
        return Ordering::Greater;
    }

    if this.post_no < other.post_no {
        return Ordering::Less;
    } else if this.post_no > other.post_no {
        return Ordering::Greater;
    }

    if this.post_sub_no < other.post_sub_no {
        return Ordering::Less;
    } else if this.post_sub_no > other.post_sub_no {
        return Ordering::Greater;
    }

    return Ordering::Equal;
}

#[test]
fn test_post_descriptor_comparison() {
    let pd1 = PostDescriptor::from_str("4chan", "a", 1, 1, 0);
    let pd2 = PostDescriptor::from_str("4chan", "a", 2, 1, 0);
    assert_eq!(Ordering::Less, compare_post_descriptors(&pd1, &pd2));

    let pd1 = PostDescriptor::from_str("4chan", "a", 2, 1, 0);
    let pd2 = PostDescriptor::from_str("4chan", "a", 1, 1, 0);
    assert_eq!(Ordering::Greater, compare_post_descriptors(&pd1, &pd2));

    let pd1 = PostDescriptor::from_str("4chan", "a", 1, 1, 0);
    let pd2 = PostDescriptor::from_str("4chan", "a", 1, 1, 0);
    assert_eq!(Ordering::Equal, compare_post_descriptors(&pd1, &pd2));

    let pd1 = PostDescriptor::from_str("4chan", "a", 1, 1, 0);
    let pd2 = PostDescriptor::from_str("4chan", "a", 1, 2, 0);
    assert_eq!(Ordering::Less, compare_post_descriptors(&pd1, &pd2));

    let pd1 = PostDescriptor::from_str("4chan", "a", 1, 2, 0);
    let pd2 = PostDescriptor::from_str("4chan", "a", 1, 1, 0);
    assert_eq!(Ordering::Greater, compare_post_descriptors(&pd1, &pd2));

    let pd1 = PostDescriptor::from_str("4chan", "a", 2, 1, 0);
    let pd2 = PostDescriptor::from_str("4chan", "a", 2, 1, 0);
    assert_eq!(Ordering::Equal, compare_post_descriptors(&pd1, &pd2));

    let pd1 = PostDescriptor::from_str("4chan", "a", 1, 1, 0);
    let pd2 = PostDescriptor::from_str("4chan", "a", 1, 1, 1);
    assert_eq!(Ordering::Less, compare_post_descriptors(&pd1, &pd2));

    let pd1 = PostDescriptor::from_str("4chan", "a", 1, 1, 1);
    let pd2 = PostDescriptor::from_str("4chan", "a", 1, 1, 0);
    assert_eq!(Ordering::Greater, compare_post_descriptors(&pd1, &pd2));

    let pd1 = PostDescriptor::from_str("4chan", "a", 1, 1, 0);
    let pd2 = PostDescriptor::from_str("4chan", "a", 1, 1, 0);
    assert_eq!(Ordering::Equal, compare_post_descriptors(&pd1, &pd2));

    let pd1 = PostDescriptor::from_str("4chan", "vg", 1, 1, 0);
    let pd2 = PostDescriptor::from_str("4chan", "a", 1, 1, 0);
    assert_eq!(Ordering::Greater, compare_post_descriptors(&pd1, &pd2));

    let pd1 = PostDescriptor::from_str("4chan", "a", 1, 1, 0);
    let pd2 = PostDescriptor::from_str("4chan", "vg", 1, 1, 0);
    assert_eq!(Ordering::Less, compare_post_descriptors(&pd1, &pd2));

    let pd1 = PostDescriptor::from_str("4chan", "a", 1, 1, 0);
    let pd2 = PostDescriptor::from_str("2ch", "a", 1, 1, 0);
    assert_eq!(Ordering::Greater, compare_post_descriptors(&pd1, &pd2));

    let pd1 = PostDescriptor::from_str("2ch", "a", 1, 1, 0);
    let pd2 = PostDescriptor::from_str("4chan", "a", 1, 1, 0);
    assert_eq!(Ordering::Less, compare_post_descriptors(&pd1, &pd2));
}