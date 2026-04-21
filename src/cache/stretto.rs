use std::sync::Arc;

use super::{CacheDriver, Counters, DefaultHasher, Key, Value};
use crate::{config::Config, parser::TraceEntry, report::Report};

#[derive(Clone)]
pub struct StrettoCache {
    config: Arc<Config>,
    // https://rust-lang.github.io/rust-clippy/master/index.html#type_complexity
    #[allow(clippy::type_complexity)]
    cache: stretto::Cache<
        Key,
        Value,
        stretto::DefaultKeyBuilder<Key>,
        stretto::DefaultCoster<Value>,
        stretto::DefaultUpdateValidator<Value>,
        stretto::DefaultCacheCallback<Value>,
        DefaultHasher,
    >,
}

impl StrettoCache {
    pub fn new(config: &Config, capacity: usize) -> Self {
        if let Some(_ttl) = config.ttl {
            todo!()
        }
        if let Some(_tti) = config.tti {
            todo!()
        }
        if config.size_aware {
            todo!()
        }

        Self {
            config: Arc::new(config.clone()),
            cache: ::stretto::Cache::builder(capacity * 10, capacity as i64)
                .set_hasher(DefaultHasher)
                // Without this, each insert is charged cost + sizeof(StoreItem<V>),
                // so the effective capacity becomes max_cost / sizeof(StoreItem<V>)
                // (~cap/57 for V = (u32, Arc<[u8]>)) — unfair vs moka whose
                // capacity counts entries.
                .set_ignore_internal_cost(true)
                .finalize()
                .unwrap(),
        }
    }

    fn get(&self, key: &usize) -> bool {
        self.cache.get(key).is_some()
    }

    fn insert(&self, key: usize, req_id: usize) {
        let value = super::make_value(&self.config, key, req_id);
        super::sleep_thread_for_insertion(&self.config);
        self.cache.insert(key, value, 1);
    }
}

impl CacheDriver<TraceEntry> for StrettoCache {
    fn get_or_insert(&mut self, entry: &TraceEntry, report: &mut Report) {
        let mut counters = Counters::default();
        let mut req_id = entry.line_number();

        for block in entry.range() {
            if self.get(&block) {
                counters.read_hit();
            } else {
                self.insert(block, req_id);
                counters.inserted();
                counters.read_missed();
            }
            req_id += 1;
        }

        counters.add_to_report(report);
    }

    fn get_or_insert_once(&mut self, _entry: &TraceEntry, _report: &mut Report) {
        unimplemented!();
    }

    fn update(&mut self, entry: &TraceEntry, report: &mut Report) {
        let mut counters = Counters::default();
        let mut req_id = entry.line_number();

        for block in entry.range() {
            self.insert(block, req_id);
            counters.inserted();
            req_id += 1;
        }

        counters.add_to_report(report);
    }
}
