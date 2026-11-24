use std::fmt;
use std::str::FromStr;

/// A one-based shard index and the total number of shards in the run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Shard {
    index: u64,
    total: u64,
}

impl Shard {
    pub fn contains(self, project_id: u64) -> bool {
        // Keep the SplitMix64 finalizer fixed so upgrades preserve cache affinity.
        let hash = (project_id ^ (project_id >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        let hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d049bb133111eb);
        let hash = hash ^ (hash >> 31);
        hash % self.total + 1 == self.index
    }
}

impl FromStr for Shard {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        const ERROR: &str = "expected INDEX/TOTAL with 1 <= INDEX <= TOTAL";

        let (index, total) = value.split_once('/').ok_or(ERROR)?;
        let index = index.parse().map_err(|_| ERROR)?;
        let total = total.parse().map_err(|_| ERROR)?;
        if index == 0 || index > total {
            return Err(ERROR);
        }
        Ok(Self { index, total })
    }
}

impl fmt::Display for Shard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.index, self.total)
    }
}

#[cfg(test)]
mod tests {
    use super::Shard;

    #[test]
    fn each_project_belongs_to_exactly_one_shard() {
        for total in [1, 3, 8] {
            for id in (1..=100).chain([u64::MAX]) {
                assert_eq!(
                    (1..=total)
                        .filter(|&index| Shard { index, total }.contains(id))
                        .count(),
                    1
                );
            }
        }
    }

    #[test]
    fn project_assignments_are_stable_across_upgrades() {
        for (project_id, index) in [(1, 2), (3, 3), (5, 1)] {
            assert!(Shard { index, total: 3 }.contains(project_id));
        }
    }
}
