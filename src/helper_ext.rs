use std::{collections::HashMap, hash::Hash};

pub trait HashMapJoin<K, V> {
    fn inner_join<V2>(self, other: HashMap<K, V2>) -> HashMap<K, (V, V2)>;
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
}
