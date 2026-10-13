use std::{collections::HashMap, path::Path, rc::Rc};

use quick_xml::{Reader, escape::resolve_predefined_entity, events::Event};

use super::{Error, cell::Cell, open_element::OpenElement};

pub fn for_each_row(
    path: &Path,
    mut on_row: impl FnMut(&[String], &[Rc<Cell>]) -> Result<(), Error>,
) -> Result<(), Error> {
    let read = |source| Error::Read {
        path: path.to_owned(),
        source,
    };
    let mut reader = Reader::from_file(path).map_err(read)?;
    let mut buffer = Vec::new();
    let mut columns: Vec<String> = Vec::new();
    let (mut in_schema, mut in_mnemonic) = (false, false);
    let mut ids: HashMap<String, Rc<Cell>> = HashMap::new();
    let mut row: Option<Vec<Rc<Cell>>> = None;
    let mut open: Vec<OpenElement> = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer).map_err(read)? {
            Event::Start(start) => match start.name().as_ref() {
                "schema" => {
                    in_schema = true;
                    columns.clear();
                },
                "mnemonic" if in_schema => in_mnemonic = true,
                "row" => row = Some(Vec::with_capacity(columns.len())),
                _ if row.is_some() => open.push(OpenElement::new(&start)?),
                _ => {},
            },
            Event::Empty(start) if row.is_some() => close(OpenElement::new(&start)?, &open, &mut row, &mut ids)?,
            Event::Text(text) => {
                let text: &str = &text;
                if in_mnemonic {
                    columns.push(text.trim().to_owned());
                } else if let Some(element) = open.last_mut() {
                    element.text.push_str(text);
                }
            },
            Event::GeneralRef(reference) => {
                let resolved = match reference.resolve_char_ref()? {
                    Some(character) => character.to_string(),
                    None => {
                        let name: &str = &reference;
                        resolve_predefined_entity(name).ok_or_else(|| Error::UnknownEntity(name.to_owned()))?.to_owned()
                    },
                };
                if let Some(element) = open.last_mut() {
                    element.text.push_str(&resolved);
                }
            },
            Event::End(end) => match end.name().as_ref() {
                "schema" => in_schema = false,
                "mnemonic" => in_mnemonic = false,
                "row" => {
                    if columns.is_empty() {
                        return Err(Error::NoSchema);
                    }
                    if let Some(cells) = row.take() {
                        on_row(&columns, &cells)?;
                    }
                },
                _ if row.is_some() => {
                    if let Some(element) = open.pop() {
                        close(element, &open, &mut row, &mut ids)?;
                    }
                },
                _ => {},
            },
            Event::Eof => break,
            _ => {},
        }
        buffer.clear();
    }
    if columns.is_empty() {
        return Err(Error::NoSchema);
    }
    Ok(())
}

fn close(
    element: OpenElement,
    open: &[OpenElement],
    row: &mut Option<Vec<Rc<Cell>>>,
    ids: &mut HashMap<String, Rc<Cell>>,
) -> Result<(), Error> {
    let cell = match element.reference {
        Some(reference) => ids.get(&reference).cloned().ok_or(Error::UnknownReference(reference))?,
        None if element.sentinel => Rc::new(Cell::default()),
        None => {
            let cell = Rc::new(Cell {
                text: element.text.trim().to_owned(),
                format: element.format,
            });
            if let Some(id) = element.id {
                ids.insert(id, cell.clone());
            }
            cell
        },
    };
    if open.is_empty()
        && let Some(cells) = row.as_mut()
    {
        cells.push(cell);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{env, fs, process};

    use super::for_each_row;

    const EXPORT: &str = r#"<?xml version="1.0"?>
<trace-query-result>
<node><schema name="syscall"><col><mnemonic>thread</mnemonic></col><col><mnemonic>call</mnemonic></col><col><mnemonic>duration</mnemonic></col><col><mnemonic>process</mnemonic></col><col><mnemonic>note</mnemonic></col></schema>
<row><thread id="2" fmt="Main Thread (cli, pid: 7)"><tid id="3" fmt="0x1">1</tid><process id="4" fmt="cli (7)"><pid id="5" fmt="7">7</pid></process></thread><syscall id="7" fmt="read">BSC_read</syscall><duration id="8" fmt="708 ns">708</duration><process ref="4"/><sentinel/></row>
<row><thread ref="2"/><syscall ref="7"/><duration id="9" fmt="1.2 ms">1200000</duration><process ref="4"/><narrative id="10" fmt="a &amp; b">x</narrative></row>
</node></trace-query-result>"#;

    #[test]
    fn resolves_references_and_nested_ids() {
        let path = env::temp_dir().join(format!("metal-perf-xctrace-test-{}.xml", process::id()));
        fs::write(&path, EXPORT).expect("write the fixture");
        let mut rows = Vec::new();
        let result = for_each_row(&path, |columns, cells| {
            assert_eq!(columns, ["thread", "call", "duration", "process", "note"]);
            rows.push(cells.iter().map(|cell| (cell.display().to_owned(), cell.number())).collect::<Vec<_>>());
            Ok(())
        });
        fs::remove_file(&path).expect("remove the fixture");
        result.expect("the fixture parses");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0].0, "Main Thread (cli, pid: 7)");
        assert_eq!(rows[0][2], ("708 ns".to_owned(), Some(708.0)));
        assert_eq!(rows[0][3].0, "cli (7)");
        assert_eq!(rows[0][4], (String::new(), None));
        assert_eq!(rows[1][1].0, "read");
        assert_eq!(rows[1][2].1, Some(1_200_000.0));
        assert_eq!(rows[1][4].0, "a & b");
    }
}
