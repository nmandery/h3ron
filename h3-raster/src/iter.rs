use std::slice::Iter;

/// iterator to zip mutiple vectors together.
///
/// returns tuples with the position and the zipped vector
pub struct ZipMultiIter<'a, T> {
    iters: Vec<Iter<'a, T>>,
    current_pos: usize,
}

impl<'a, T> ZipMultiIter<'a, T> {
    pub fn new(vectors: &'a [Vec<T>]) -> ZipMultiIter<'a, T> {
        ZipMultiIter {
            iters: vectors.iter().map(|v| v.iter()).collect(),
            current_pos: 0,
        }
    }
}

impl<'a, T> Iterator for ZipMultiIter<'a, T> {
    type Item = (usize, Vec<&'a T>);

    fn next(&mut self) -> Option<Self::Item> {
        let num_elements = self.iters.len();
        let mut row = Vec::new();
        row.reserve(num_elements);
        for it in self.iters.iter_mut() {
            match it.next() {
                Some(v) => row.push(v),
                None => return None, // one iterator reached its end
            }
        }
        self.current_pos += 1;
        Some((self.current_pos - 1, row))
    }
}

#[cfg(test)]
mod tests {
    use crate::iter::ZipMultiIter;

    #[test]
    fn test_zip_multi_iter_2() {
        let vecs = vec![
            vec![1, 2, 3, 4],
            vec![5, 6, 7],
        ];
        let mut zmi = ZipMultiIter::new(&vecs);
        assert_eq!(zmi.next(), Some((0, vec![&1, &5])));
        assert_eq!(zmi.next(), Some((1, vec![&2, &6])));
        assert_eq!(zmi.next(), Some((2, vec![&3, &7])));
        assert_eq!(zmi.next(), None);
    }

    #[test]
    fn test_zip_multi_iter_3() {
        let vecs = vec![
            vec![1, 2, 3, 4],
            vec![9, 10],
            vec![5, 6, 7],
        ];
        let mut zmi = ZipMultiIter::new(&vecs);
        assert_eq!(zmi.next(), Some((0, vec![&1, &9, &5])));
        assert_eq!(zmi.next(), Some((1, vec![&2, &10, &6])));
        assert_eq!(zmi.next(), None);
    }

    #[test]
    fn test_zip_multi_iter_2_with_filter() {
        let vecs = vec![
            vec![1, 2, 3, 4],
            vec![5, 6, 7],
        ];
        let mut zmi = ZipMultiIter::new(&vecs).filter(
            |(_u, v)| v.iter().any(|&i| i > &6)
        );
        assert_eq!(zmi.next(), Some((2, vec![&3, &7])));
        assert_eq!(zmi.next(), None);
    }
}
