use std::{collections::HashMap, hash::Hash};

pub trait HashMapJoin<K, V> {
    fn inner_join<V2>(self, other: HashMap<K, V2>) -> HashMap<K, (V, V2)>;
    fn outer_join<V2>(self, other: HashMap<K, V2>) -> HashMap<K, (Option<V>, Option<V2>)>;
}

impl<K, V> HashMapJoin<K, V> for HashMap<K, V>
where
    K: Eq + Hash,
{
    fn inner_join<V2>(self, mut other: HashMap<K, V2>) -> HashMap<K, (V, V2)> {
        let mut result = HashMap::new();
        for (key, value) in self {
            if let Some(other_value) = other.remove(&key) {
                result.insert(key, (value, other_value));
            }
        }
        result
    }

    fn outer_join<V2>(self, mut other: HashMap<K, V2>) -> HashMap<K, (Option<V>, Option<V2>)> {
        let mut result = HashMap::new();

        for (key, value) in self {
            let other_value = other.remove(&key);
            result.insert(key, (Some(value), other_value));
        }

        for (key, value) in other {
            result.entry(key).or_insert((None, Some(value)));
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_inner_join_no_overlap() {
        let map1: HashMap<i32, i32> = [(1, 10), (2, 20)].iter().cloned().collect();
        let map2: HashMap<i32, String> = [(3, "thirty".to_string()), (4, "forty".to_string())]
            .iter()
            .cloned()
            .collect();

        let result = map1.inner_join(map2);
        assert!(result.is_empty());
    }

    #[test]
    fn test_inner_join_some_overlap() {
        let map1: HashMap<i32, i32> = [(1, 10), (2, 20), (3, 30)].iter().cloned().collect();
        let map2: HashMap<i32, String> = [(2, "twenty".to_string()), (3, "thirty".to_string())]
            .iter()
            .cloned()
            .collect();

        let result = map1.inner_join(map2);
        assert_eq!(result.len(), 2);
        assert_eq!(result.get(&2), Some(&(20, "twenty".to_string())));
        assert_eq!(result.get(&3), Some(&(30, "thirty".to_string())));
    }

    #[test]
    fn test_inner_join_one_empty() {
        let map1: HashMap<i32, i32> = HashMap::new();
        let map2: HashMap<i32, String> = [(1, "ten".to_string()), (2, "twenty".to_string())]
            .iter()
            .cloned()
            .collect();

        let result = map1.inner_join(map2);
        assert!(result.is_empty());
    }

    #[test]
    fn test_outer_join_no_overlap() {
        let map1: HashMap<i32, i32> = [(1, 10), (2, 20)].iter().cloned().collect();
        let map2: HashMap<i32, String> = [(3, "thirty".to_string()), (4, "forty".to_string())]
            .iter()
            .cloned()
            .collect();

        let result = map1.outer_join(map2);
        assert_eq!(result.len(), 4);
        assert_eq!(result.get(&1), Some(&(Some(10), None)));
        assert_eq!(result.get(&2), Some(&(Some(20), None)));
        assert_eq!(result.get(&3), Some(&(None, Some("thirty".to_string()))));
        assert_eq!(result.get(&4), Some(&(None, Some("forty".to_string()))));
    }

    #[test]
    fn test_outer_join_some_overlap() {
        let map1: HashMap<i32, i32> = [(1, 10), (2, 20), (3, 30)].iter().cloned().collect();
        let map2: HashMap<i32, String> = [
            (2, "twenty".to_string()),
            (3, "thirty".to_string()),
            (4, "forty".to_string()),
        ]
        .iter()
        .cloned()
        .collect();

        let result = map1.outer_join(map2);
        assert_eq!(result.len(), 4);
        assert_eq!(result.get(&1), Some(&(Some(10), None)));
        assert_eq!(
            result.get(&2),
            Some(&(Some(20), Some("twenty".to_string())))
        );
        assert_eq!(
            result.get(&3),
            Some(&(Some(30), Some("thirty".to_string())))
        );
        assert_eq!(result.get(&4), Some(&(None, Some("forty".to_string()))));
    }

    #[test]
    fn test_outer_join_one_empty() {
        let map1: HashMap<i32, i32> = HashMap::new();
        let map2: HashMap<i32, String> = [(1, "ten".to_string()), (2, "twenty".to_string())]
            .iter()
            .cloned()
            .collect();

        let result = map1.outer_join(map2);
        assert_eq!(result.len(), 2);
        assert_eq!(result.get(&1), Some(&(None, Some("ten".to_string()))));
        assert_eq!(result.get(&2), Some(&(None, Some("twenty".to_string()))));
    }

    #[test]
    fn test_inner_join_duplicate_keys_diff_values() {
        let map1: HashMap<i32, i32> = [(1, 10), (2, 20), (3, 30)].iter().cloned().collect();
        let map2: HashMap<i32, String> = [
            (1, "ten".to_string()),
            (2, "twenty different".to_string()),
            (3, "thirty".to_string()),
        ]
        .iter()
        .cloned()
        .collect();

        let result = map1.inner_join(map2);
        assert_eq!(result.len(), 3);
        assert_eq!(result.get(&1), Some(&(10, "ten".to_string())));
        assert_eq!(result.get(&2), Some(&(20, "twenty different".to_string())));
        assert_eq!(result.get(&3), Some(&(30, "thirty".to_string())));
    }

    #[test]
    fn test_outer_join_duplicate_keys_diff_values() {
        let map1: HashMap<i32, i32> = [(1, 10), (2, 20), (3, 30)].iter().cloned().collect();
        let map2: HashMap<i32, String> = [
            (1, "ten".to_string()),
            (2, "twenty different".to_string()),
            (3, "thirty".to_string()),
            (4, "forty".to_string()),
        ]
        .iter()
        .cloned()
        .collect();

        let result = map1.outer_join(map2);
        assert_eq!(result.len(), 4);
        assert_eq!(result.get(&1), Some(&(Some(10), Some("ten".to_string()))));
        assert_eq!(
            result.get(&2),
            Some(&(Some(20), Some("twenty different".to_string())))
        );
        assert_eq!(
            result.get(&3),
            Some(&(Some(30), Some("thirty".to_string())))
        );
        assert_eq!(result.get(&4), Some(&(None, Some("forty".to_string()))));
    }
}
