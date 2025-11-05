use std::collections::{HashMap, VecDeque};

use crate::utils::hash::murmur2_hash;

/// Options configuring cache behaviour.
#[derive(Debug, Clone)]
pub struct CacheOptions {
    pub cache: bool,
    pub max_size: usize,
}

impl Default for CacheOptions {
    fn default() -> Self {
        Self {
            cache: true,
            max_size: 500,
        }
    }
}

/// Simple LRU cache mirroring the behaviour of the Babel implementation.
#[derive(Debug)]
pub struct Cache<T> {
    options: CacheOptions,
    entries: HashMap<String, T>,
    order: VecDeque<String>,
}

impl<T> Cache<T> {
    pub fn new() -> Self {
        Self {
            options: CacheOptions::default(),
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    /// Configures the cache options.
    pub fn initialize(&mut self, options: CacheOptions) {
        self.options = options;
    }

    /// Creates a deterministic cache key combining the namespace and key.
    pub fn unique_key(cache_key: &str, namespace: Option<&str>) -> String {
        match namespace {
            Some(ns) if !ns.is_empty() => murmur2_hash(&format!("{ns}----{cache_key}"), 0),
            _ => murmur2_hash(cache_key, 0),
        }
    }

    /// Loads a value from the cache, computing and inserting it on miss.
    pub fn load<F>(&mut self, namespace: Option<&str>, cache_key: &str, value: F) -> T
    where
        T: Clone,
        F: FnOnce() -> T,
    {
        if !self.options.cache {
            return value();
        }

        if let Some(existing) = self.get(namespace, cache_key) {
            return existing;
        }

        let computed = value();
        self.insert(namespace, cache_key, computed.clone());
        computed
    }

    /// Retrieves a cached entry if present.
    pub fn get(&mut self, namespace: Option<&str>, cache_key: &str) -> Option<T>
    where
        T: Clone,
    {
        let unique_key = Self::unique_key(cache_key, namespace);
        if let Some(existing) = self.entries.get(&unique_key).cloned() {
            self.promote_key(&unique_key);
            Some(existing)
        } else {
            None
        }
    }

    /// Inserts a value into the cache respecting the configured size.
    pub fn insert(&mut self, namespace: Option<&str>, cache_key: &str, value: T)
    where
        T: Clone,
    {
        if !self.options.cache {
            return;
        }

        let unique_key = Self::unique_key(cache_key, namespace);

        if !self.entries.contains_key(&unique_key) && self.entries.len() >= self.options.max_size {
            if let Some(front) = self.order.pop_front() {
                self.entries.remove(&front);
            }
        }

        self.entries.insert(unique_key.clone(), value);
        self.promote_key(&unique_key);
    }

    fn promote_key(&mut self, unique_key: &str) {
        if let Some(position) = self
            .order
            .iter()
            .position(|existing| existing == unique_key)
        {
            self.order.remove(position);
        }

        self.order.push_back(unique_key.to_owned());
    }
}

impl<T> Default for Cache<T> {
    fn default() -> Self {
        Self::new()
    }
}
