use itertools::Itertools;

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

pub trait MedianStat<T> {
    fn median(self) -> Option<T>;
}

impl<T, I> MedianStat<T> for I
where
    I: Iterator<Item = T>,
    T: PartialOrd + Clone,
{
    fn median(self) -> Option<T> {
        let sorted = self
            .sorted_by(|x, y| x.partial_cmp(y).unwrap())
            .collect::<Vec<_>>();
        if !sorted.is_empty() {
            let ind = sorted.len() / 2;
            Some(sorted[ind].clone())
        } else {
            None
        }
    }
}
