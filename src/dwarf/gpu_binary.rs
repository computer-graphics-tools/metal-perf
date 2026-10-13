use std::{collections::HashMap, path::PathBuf, rc::Rc};

use gimli::{
    AttributeValue, DW_AT_abstract_origin, DW_AT_linkage_name, DW_AT_name, DW_AT_specification,
    DW_TAG_inlined_subroutine, DW_TAG_subprogram, DebuggingInformationEntry, EntriesTreeNode, RunTimeEndian, Unit,
};
use object::{Object, ObjectSection};

use super::{Error, Reader, inline_bytes::InlineBytes, line_span::LineSpan, range_set::RangeSet};

pub struct GpuBinary<'data> {
    dwarf: gimli::Dwarf<Reader<'data>>,
}

impl<'data> GpuBinary<'data> {
    pub fn parse(data: &'data [u8]) -> Result<Self, Error> {
        let file = object::File::parse(data)?;
        let endian = if file.is_little_endian() {
            RunTimeEndian::Little
        } else {
            RunTimeEndian::Big
        };
        if file.section_by_name(".debug_line").is_none() {
            return Err(Error::NoLineTable);
        }
        let dwarf = gimli::Dwarf::load(|section| -> Result<Reader<'data>, Error> {
            let bytes = match file.section_by_name(section.name()) {
                Some(section) => section.data()?,
                None => &[],
            };
            Ok(gimli::EndianSlice::new(bytes, endian))
        })?;
        Ok(Self {
            dwarf,
        })
    }

    pub fn line_spans(&self) -> Result<Vec<LineSpan>, Error> {
        let mut spans = Vec::new();
        let mut headers = self.dwarf.units();
        while let Some(header) = headers.next()? {
            let unit = self.dwarf.unit(header)?;
            let Some(program) = unit.line_program.clone() else {
                continue;
            };
            let mut files: HashMap<u64, Rc<str>> = HashMap::new();
            let mut open: Option<(u64, Rc<str>, u64)> = None;
            let mut rows = program.rows();
            while let Some((header, row)) = rows.next_row()? {
                if let Some((address, file, line)) = open.take()
                    && row.address() > address
                {
                    spans.push(LineSpan {
                        address,
                        size: row.address() - address,
                        file,
                        line,
                    });
                }
                if row.end_sequence() {
                    continue;
                }
                let file = match files.get(&row.file_index()) {
                    Some(file) => file.clone(),
                    None => {
                        let mut path = PathBuf::new();
                        if let Some(entry) = row.file(header) {
                            if let Some(directory) = entry.directory(header) {
                                path.push(&*self.dwarf.attr_string(&unit, directory)?.to_string_lossy());
                            }
                            path.push(&*self.dwarf.attr_string(&unit, entry.path_name())?.to_string_lossy());
                        }
                        let file: Rc<str> = path.to_string_lossy().into();
                        files.insert(row.file_index(), file.clone());
                        file
                    },
                };
                open = Some((row.address(), file, row.line().map_or(0, |line| line.get())));
            }
        }
        spans.sort_by_key(|span| span.address);
        Ok(spans)
    }

    pub fn inline_bytes(&self) -> Result<InlineBytes, Error> {
        let mut bytes = InlineBytes::default();
        let mut headers = self.dwarf.units();
        while let Some(header) = headers.next()? {
            let unit = self.dwarf.unit(header)?;
            let mut tree = unit.entries_tree(None)?;
            self.visit(&unit, tree.root()?, None, &mut bytes)?;
        }
        Ok(bytes)
    }

    fn visit(
        &self,
        unit: &Unit<Reader<'data>>,
        node: EntriesTreeNode<Reader<'data>>,
        owner: Option<&str>,
        bytes: &mut InlineBytes,
    ) -> Result<RangeSet, Error> {
        let entry = node.entry().clone();
        let mut ranges = Vec::new();
        let mut iterator = self.dwarf.die_ranges(unit, &entry)?;
        while let Some(range) = iterator.next()? {
            ranges.push((range.begin, range.end));
        }
        let ranges = RangeSet::new(ranges);
        let is_function = entry.tag() == DW_TAG_subprogram || entry.tag() == DW_TAG_inlined_subroutine;
        let name = (is_function && !ranges.is_empty()).then(|| self.name(unit, &entry)).transpose()?;
        if let Some(name) = &name
            && owner.is_none()
        {
            bytes.kernels.push((name.clone(), ranges.len()));
        }
        let owner = name.as_deref().or(owner);
        let mut covered = Vec::new();
        let mut children = node.children();
        while let Some(child) = children.next()? {
            covered.extend(self.visit(unit, child, owner, bytes)?.into_ranges());
        }
        let covered = RangeSet::new(covered);
        if ranges.is_empty() {
            return Ok(covered);
        }
        if let Some(owner) = owner {
            *bytes.exclusive.entry(owner.to_owned()).or_default() += ranges.len() - ranges.intersection_len(&covered);
        }
        Ok(ranges)
    }

    fn name(
        &self,
        unit: &Unit<Reader<'data>>,
        entry: &DebuggingInformationEntry<Reader<'data>>,
    ) -> Result<String, Error> {
        for attribute in [DW_AT_name, DW_AT_linkage_name] {
            if let Some(value) = entry.attr_value(attribute) {
                return Ok(self.dwarf.attr_string(unit, value)?.to_string_lossy().into_owned());
            }
        }
        for attribute in [DW_AT_abstract_origin, DW_AT_specification] {
            if let Some(AttributeValue::UnitRef(offset)) = entry.attr_value(attribute) {
                return self.name(unit, &unit.entry(offset)?);
            }
        }
        Ok(entry.tag().static_string().unwrap_or("?").to_owned())
    }
}
