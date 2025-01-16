use std::fmt::Debug;

#[derive(Debug)]
pub(super) struct InplaceFlatMapper<'a, T> {
    data: &'a mut Vec<T>,
    read_pos: usize,
    write_pos: usize,
    cache: Vec<T>,
    cache_pos: usize,
}

pub trait NextInPlaceExt<T> {
    fn flat_map_inplace<TMap: for<'b> FnMut(T, &mut InplaceFlatMapper<'b, T>)>(
        &mut self,
        handler: TMap,
    );
}

impl<T: Copy + Debug> NextInPlaceExt<T> for Vec<T> {
    fn flat_map_inplace<TMap: for<'b> FnMut(T, &mut InplaceFlatMapper<'b, T>)>(
        &mut self,
        handler: TMap,
    ) {
        let mut mapper = InplaceFlatMapper::new(self);
        mapper.map(handler)
    }
}

impl<'a, T: Copy + std::fmt::Debug> InplaceFlatMapper<'a, T> {
    fn new(data: &'a mut Vec<T>) -> Self {
        Self {
            data,
            read_pos: 0,
            write_pos: 0,
            cache: Default::default(),
            cache_pos: 0,
        }
    }
    fn map(&mut self, mut handler: impl for<'b> FnMut(T, &mut InplaceFlatMapper<'b, T>)) {
        while self.read_pos < self.data.len() {
            let el = self.data[self.read_pos];
            self.read_pos += 1;
            handler(el, self);
            self.cache_pos = 0;
            while self.cache_pos < self.cache.len() {
                let el = self.cache[self.cache_pos];
                self.cache_pos += 1;
                handler(el, self)
            }
        }
        self.data.truncate(self.write_pos);
    }
    pub fn insert(&mut self, el: T) {
        if self.read_pos == self.write_pos {
            let remaining_read = self.data.len() - self.read_pos;
            let to_cache_len = BATCH_TO_CACHE.min(remaining_read);
            let to_cache = &self.data[self.read_pos..self.read_pos + to_cache_len];
            let remaining_in_cache = self.cache.len() - self.cache_pos;

            if remaining_in_cache * 8 < self.cache.len() {
                self.cache.drain(0..self.cache_pos);
                self.cache_pos = 0;
            }
            self.cache.extend_from_slice(to_cache);
            self.read_pos += to_cache_len;
        }
        if let Some(x) = self.data.get_mut(self.write_pos) {
            *x = el;
        } else {
            self.data.push(el);
            self.read_pos += 1;
        }
        self.write_pos += 1;
    }
}

const BATCH_TO_CACHE: usize = 10;

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn extender_duplicate_twice() {
        let range = 0..1000;
        let mut data = range.clone().collect::<Vec<_>>();
        data.flat_map_inplace(|e, inserter| {
            inserter.insert(e);
            inserter.insert(e);
        });
        assert_eq!(range.flat_map(|x| [x, x]).collect::<Vec<_>>(), data);
    }
    #[test]
    fn only_use_every_second() {
        let mut data = vec![0, 1, 2, 3];
        InplaceFlatMapper::new(&mut data).map(|el, inserter| {
            if el % 2 == 0 {
                inserter.insert(el);
            }
        });
        assert_eq!(vec![0, 2], data);
    }
    #[test]
    fn only_use_every_second_twice() {
        let mut data = vec![0, 1, 2, 3];
        InplaceFlatMapper::new(&mut data).map(|el, inserter| {
            if el % 2 == 0 {
                inserter.insert(el);
                inserter.insert(el);
            }
        });
        assert_eq!(vec![0, 0, 2, 2], data);
    }
    #[test]
    fn assert_low_cache_capacity() {
        let range = 0..100;
        let mut data = range.clone().collect();
        let mut mapper = InplaceFlatMapper::new(&mut data);
        mapper.cache.shrink_to_fit();
        mapper.map(|el, inserter| {
            if el % 2 == 0 {
                inserter.insert(el);
                inserter.insert(el);
            }
        });
        assert_eq!(
            &range
                .filter(|x| x % 2 == 0)
                .flat_map(|x| [x, x])
                .collect::<Vec<_>>(),
            mapper.data
        );
        assert!(
            mapper.cache.capacity() < 20,
            "Expected small cache: {}",
            mapper.cache.capacity()
        );
    }
}
