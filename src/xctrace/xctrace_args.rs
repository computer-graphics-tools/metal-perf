use std::{collections::HashMap, path::PathBuf, rc::Rc};

use clap::Args;

use super::{Error, cell::Cell, filter::Filter, minimum::Minimum, table::for_each_row};

#[derive(Debug, Args)]
pub struct XctraceArgs {
    /// XML written by `xctrace export --input X.trace --xpath '/trace-toc/run/data/table[@schema="..."]'`
    export: PathBuf,
    /// Keep rows whose displayed column value contains text, as column~text; repeat to require all
    #[arg(long = "filter")]
    filters: Vec<Filter>,
    /// Keep rows whose numeric column value is at least a number, as column=number; repeat to require all
    #[arg(long = "min")]
    minimums: Vec<Minimum>,
    /// Columns whose displayed values form the group key
    #[arg(long, value_delimiter = ',')]
    group_by: Vec<String>,
    /// Numeric columns summed per group, such as duration
    #[arg(long, value_delimiter = ',')]
    sum: Vec<String>,
    /// Groups listed, largest first
    #[arg(long, default_value_t = 20)]
    top: usize,
}

impl XctraceArgs {
    pub fn run(&self) -> Result<(), Error> {
        let (mut filter_columns, mut minimum_columns, mut group_columns, mut sum_columns) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut columns_seen: Vec<String> = Vec::new();
        let mut groups: HashMap<Vec<String>, (usize, Vec<f64>)> = HashMap::new();
        let (mut total, mut kept) = (0usize, 0usize);
        for_each_row(&self.export, |columns, cells| {
            total += 1;
            if columns_seen != columns {
                columns_seen = columns.to_vec();
                let find = |names: &mut dyn Iterator<Item = &String>| -> Result<Vec<usize>, Error> {
                    names
                        .map(|name| {
                            columns.iter().position(|column| column == name).ok_or_else(|| Error::UnknownColumn {
                                name: name.clone(),
                                columns: columns.join(", "),
                            })
                        })
                        .collect()
                };
                filter_columns = find(&mut self.filters.iter().map(|filter| &filter.column))?;
                minimum_columns = find(&mut self.minimums.iter().map(|minimum| &minimum.column))?;
                group_columns = find(&mut self.group_by.iter())?;
                sum_columns = find(&mut self.sum.iter())?;
            }
            let cell = |index: usize| cells.get(index).map(Rc::as_ref).unwrap_or(&EMPTY);
            let matches = self
                .filters
                .iter()
                .zip(&filter_columns)
                .all(|(filter, &index)| cell(index).display().contains(&filter.text))
                && self
                    .minimums
                    .iter()
                    .zip(&minimum_columns)
                    .all(|(minimum, &index)| cell(index).number().is_some_and(|value| value >= minimum.value));
            if !matches {
                return Ok(());
            }
            kept += 1;
            let key: Vec<String> = group_columns.iter().map(|&index| cell(index).display().to_owned()).collect();
            let group = groups.entry(key).or_insert_with(|| (0, vec![0.0; sum_columns.len()]));
            group.0 += 1;
            for (slot, &index) in group.1.iter_mut().zip(&sum_columns) {
                *slot += cell(index).number().unwrap_or(0.0);
            }
            Ok(())
        })?;

        println!("columns: {}", columns_seen.join(", "));
        println!("rows: {total}, kept: {kept}");
        let mut groups: Vec<_> = groups.into_iter().collect();
        groups.sort_by(|left, right| {
            let weight =
                |group: &(Vec<String>, (usize, Vec<f64>))| group.1.1.first().copied().unwrap_or(group.1.0 as f64);
            weight(right).total_cmp(&weight(left)).then_with(|| left.0.cmp(&right.0))
        });
        let sums: String = self.sum.iter().map(|name| format!(" {name:>16}")).collect();
        println!("{:>9}{sums}  {}", "count", self.group_by.join(" | "));
        for (key, (count, totals)) in groups.iter().take(self.top) {
            let totals: String = totals.iter().map(|value| format!(" {value:>16.0}")).collect();
            println!("{count:>9}{totals}  {}", key.join(" | "));
        }
        Ok(())
    }
}

static EMPTY: Cell = Cell {
    text: String::new(),
    format: None,
};
