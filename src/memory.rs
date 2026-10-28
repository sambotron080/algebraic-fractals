//! Renders kept in memory, so that the same search never runs twice: the explorer's
//! Back and the video window's waypoint strip and frame preview all look here first.

use crate::engine::{Coefs, Render};
use std::sync::Arc;

/// Keeps renders up to this many bytes.
pub const RENDER_MEMORY: usize = 3 << 30;

/// What a render searched: the order, the coefficients (with their max) and the exact
/// sample points (as bits). A render with the same key is the same search.
#[derive(Clone, PartialEq, Debug)]
pub struct RenderKey {
    order: usize,
    coefs: Coefs,
    re: Vec<u64>,
    im: Vec<u64>,
}

impl RenderKey {
    pub fn new(order: usize, coefs: Coefs, re: &[f64], im: &[f64]) -> RenderKey {
        RenderKey { order, coefs, re: re.iter().map(|v| v.to_bits()).collect(), im: im.iter().map(|v| v.to_bits()).collect() }
    }
}

/// Up to `limit` bytes of renders, the least recently used dropped first.
pub struct RenderMemory {
    /// latest used last
    kept: Vec<(RenderKey, Arc<Render>)>,
    bytes: usize,
    pub limit: usize,
}

impl Default for RenderMemory {
    fn default() -> RenderMemory {
        RenderMemory { kept: Vec::new(), bytes: 0, limit: RENDER_MEMORY }
    }
}

pub fn render_bytes(r: &Render) -> usize {
    8 * (r.re_axis.len() + r.im_axis.len() + r.residual.len()) + 4 * r.coeffs.len()
}

impl RenderMemory {
    pub fn get(&mut self, key: &RenderKey) -> Option<Arc<Render>> {
        let i = self.kept.iter().position(|(k, _)| k == key)?;
        let entry = self.kept.remove(i);
        let r = entry.1.clone();
        self.kept.push(entry);
        Some(r)
    }

    pub fn put(&mut self, key: RenderKey, r: Arc<Render>) {
        if let Some(i) = self.kept.iter().position(|(k, _)| *k == key) {
            self.bytes -= render_bytes(&self.kept.remove(i).1);
        }
        self.bytes += render_bytes(&r);
        self.kept.push((key, r));
        while self.bytes > self.limit && self.kept.len() > 1 {
            self.bytes -= render_bytes(&self.kept.remove(0).1);
        }
    }

    pub fn len(&self) -> usize {
        self.kept.len()
    }

    pub fn is_empty(&self) -> bool {
        self.kept.is_empty()
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{render_axes, Plan, Region};
    use std::sync::atomic::{AtomicBool, AtomicUsize};

    /// Renders kept in memory are found by exactly what was searched, and the least
    /// recently used go first when there are too many.
    #[test]
    fn render_memory() {
        let plan = Plan::new(3, &Coefs::ints(4), false).unwrap();
        let region = Region { re: (0.1, 0.5), im: (0.2, 0.45), width: 16 };
        let (re, im) = region.axes();
        let r = Arc::new(render_axes(&plan, re.clone(), im.clone(), 2, &AtomicBool::new(false), &AtomicUsize::new(0)).unwrap());
        let key = |max: i32, re: &[f64]| RenderKey::new(3, Coefs::ints(max), re, &im);
        let mut m = RenderMemory::default();
        m.put(key(4, &re), r.clone());
        assert!(m.get(&key(4, &re)).is_some_and(|got| Arc::ptr_eq(&got, &r)));
        assert!(m.get(&key(5, &re)).is_none(), "another max coefficient");
        let moved: Vec<f64> = re.iter().map(|v| v + 1e-17).collect();
        assert!(moved != re && m.get(&key(4, &moved)).is_none(), "other sample points");
        m.put(key(4, &re), r.clone());
        assert_eq!((m.len(), m.bytes()), (1, render_bytes(&r)), "putting it again replaces it");
        // fill memory with renders of other max coefficients: the first one used least
        // recently goes first, and the total stays within the limit
        let per = render_bytes(&r);
        m.limit = 10 * per + per / 2;
        let fit = 10;
        for max in 5..5 + fit as i32 {
            m.put(key(max, &re), r.clone());
            if max == 6 {
                assert!(m.get(&key(4, &re)).is_some()); // used again, so kept longer
            }
        }
        assert!(m.bytes() <= m.limit && m.len() == fit);
        assert!(m.get(&key(4, &re)).is_some() && m.get(&key(5, &re)).is_none());
    }
}
