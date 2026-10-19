#[derive(Clone)]
pub struct RangeSet {
    ranges: Vec<(u64, u64)>,
}

impl RangeSet {
    pub fn new(mut ranges: Vec<(u64, u64)>) -> Self {
        ranges.retain(|(begin, end)| begin < end);
        ranges.sort_unstable();
        let mut merged: Vec<(u64, u64)> = Vec::with_capacity(ranges.len());
        for (begin, end) in ranges {
            match merged.last_mut() {
                Some(last) if begin <= last.1 => last.1 = last.1.max(end),
                _ => merged.push((begin, end)),
            }
        }
        Self {
            ranges: merged,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub fn len(&self) -> u64 {
        self.ranges.iter().map(|(begin, end)| end - begin).sum()
    }

    pub fn intersection_len(
        &self,
        other: &Self,
    ) -> u64 {
        let (mut left, mut right, mut total) = (0, 0, 0);
        while left < self.ranges.len() && right < other.ranges.len() {
            let (a, b) = (self.ranges[left], other.ranges[right]);
            total += b.1.min(a.1).saturating_sub(a.0.max(b.0));
            if a.1 < b.1 {
                left += 1;
            } else {
                right += 1;
            }
        }
        total
    }

    pub fn into_ranges(self) -> Vec<(u64, u64)> {
        self.ranges
    }
}

#[cfg(test)]
mod tests {
    use super::RangeSet;

    #[test]
    fn merges_and_intersects() {
        let parent = RangeSet::new(vec![(0, 100), (90, 120), (200, 210), (5, 5)]);
        assert_eq!(parent.clone().into_ranges(), [(0, 120), (200, 210)]);
        assert_eq!(parent.len(), 130);
        let children = RangeSet::new(vec![(10, 20), (110, 205), (300, 400)]);
        assert_eq!(parent.intersection_len(&children), 10 + 10 + 5);
        assert!(RangeSet::new(vec![(3, 1)]).is_empty());
    }
}
