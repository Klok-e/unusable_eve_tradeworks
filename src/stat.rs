use itertools::Itertools;
use ordered_float::NotNan;

pub trait AverageStat {
    fn average(self) -> Option<f64>;
}

impl<I> AverageStat for I
where
    I: Iterator<Item = f64>,
{
    fn average(self) -> Option<f64> {
        let mut count = 0.;
        let mut sum = 0.;
        for it in self {
            sum += it;
            count += 1.;
        }
        if count > 0. {
            Some(sum / count)
        } else {
            None
        }
    }
}

pub trait MedianStat {
    fn median(self) -> Option<NotNan<f64>>;
}

impl<I> MedianStat for I
where
    I: Iterator<Item = NotNan<f64>>,
{
    fn median(self) -> Option<NotNan<f64>> {
        let sorted = self.sorted_by(|x, y| x.cmp(y)).collect::<Vec<_>>();
        if !sorted.is_empty() {
            if sorted.len() % 2 == 0 {
                let val = (sorted[sorted.len() / 2] + sorted[sorted.len() / 2 + 1]) / 2.;
                Some(val)
            } else {
                Some(sorted[(sorted.len() / 2)])
            }
        } else {
            None
        }
    }
}
