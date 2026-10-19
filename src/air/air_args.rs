use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    process::{self, Command},
};

use clap::Args;
use object::read::archive::ArchiveFile;
use regex::Regex;

use super::{Error, ir_module::IrModule};
use crate::text::truncate;

const NAME_WIDTH: usize = 160;

#[derive(Debug, Args)]
pub struct AirArgs {
    /// A .metallib, .air, AIR archive (.rtlib, .a) or LLVM IR text (.ll)
    input: PathBuf,
    /// Regex over defined function names whose call trees to print [default: a module summary]
    #[arg(long)]
    function: Option<Regex>,
    /// Levels of defined callees to expand below each selected function
    #[arg(long, default_value_t = 1)]
    depth: usize,
    /// For archives: start from members whose name or bytes contain this text, such as a symbol name;
    /// callees are then pulled in through the archive symbol table
    #[arg(long)]
    member: Option<String>,
    /// Also write the disassembled IR to this file
    #[arg(long)]
    dump: Option<PathBuf>,
    /// Rows listed in summaries
    #[arg(long, default_value_t = 40)]
    top: usize,
}

impl AirArgs {
    pub fn run(&self) -> Result<(), Error> {
        let text = self.disassemble()?;
        if let Some(path) = &self.dump {
            fs::write(path, &text).map_err(|source| Error::Write {
                path: path.clone(),
                source,
            })?;
        }
        let module = IrModule::parse(&text);
        let Some(pattern) = &self.function else {
            self.summarize(&module);
            return Ok(());
        };
        let selected: Vec<_> = module.functions().iter().filter(|function| pattern.is_match(&function.name)).collect();
        if selected.is_empty() {
            return Err(Error::NoFunction(pattern.to_string()));
        }
        for function in selected {
            println!("{} ({} instructions)", function.name, function.instructions);
            let mut totals = HashMap::new();
            module.print_calls(&function.name, 1, self.depth, &mut Vec::new(), &mut totals);
            println!("  air.* call sites within depth {}, weighted by call multiplicity:", self.depth);
            print_counts(totals, self.top, 4);
            println!();
        }
        Ok(())
    }

    fn summarize(
        &self,
        module: &IrModule,
    ) {
        let mut functions: Vec<_> = module.functions().iter().collect();
        functions
            .sort_by(|left, right| right.instructions.cmp(&left.instructions).then_with(|| left.name.cmp(&right.name)));
        println!("{} defined functions, largest first:", functions.len());
        for function in functions.iter().take(self.top) {
            println!("  {:7} {}", function.instructions, truncate(&function.name, NAME_WIDTH));
        }
        let mut totals: HashMap<String, usize> = HashMap::new();
        for function in module.functions() {
            for (callee, count) in function.calls.iter().filter(|(callee, _)| callee.starts_with("air.")) {
                *totals.entry(callee.clone()).or_default() += count;
            }
        }
        println!("air.* call sites in the module:");
        print_counts(totals, self.top, 2);
    }

    fn disassemble(&self) -> Result<String, Error> {
        let read = |source| Error::Read {
            path: self.input.clone(),
            source,
        };
        if self.input.extension().is_some_and(|extension| extension == "ll") {
            return fs::read_to_string(&self.input).map_err(read);
        }
        let data = fs::read(&self.input).map_err(read)?;
        if !data.starts_with(b"!<arch>\n") {
            return objdump(&self.input);
        }
        let staging = env::temp_dir().join(format!("metal-perf-air-{}", process::id()));
        fs::create_dir_all(&staging).map_err(|source| Error::Write {
            path: staging.clone(),
            source,
        })?;
        let text = self.disassemble_members(&data, &staging);
        let removed = fs::remove_dir_all(&staging).map_err(|source| Error::Write {
            path: staging,
            source,
        });
        let text = text?;
        removed?;
        Ok(text)
    }

    fn disassemble_members(
        &self,
        data: &[u8],
        staging: &Path,
    ) -> Result<String, Error> {
        let needle =
            self.member.as_deref().map(|member| regex::bytes::Regex::new(&regex::escape(member))).transpose()?;
        let archive = ArchiveFile::parse(data)?;
        let mut loaded = HashSet::new();
        let mut text = String::new();
        for member in archive.members() {
            let member = member?;
            let bytes = member.data(data)?;
            if needle.as_ref().is_none_or(|needle| needle.is_match(member.name()) || needle.is_match(bytes)) {
                load_member(&mut text, &mut loaded, member.name(), bytes, staging)?;
            }
        }
        if text.is_empty() {
            return Err(Error::NoMember(self.member.clone().unwrap_or_default()));
        }
        let Some(pattern) = &self.function else {
            return Ok(text);
        };
        let mut symbols = HashMap::new();
        for symbol in archive.symbols()?.into_iter().flatten() {
            let symbol = symbol?;
            symbols.insert(String::from_utf8_lossy(symbol.name()).into_owned(), symbol.offset());
        }
        loop {
            let module = IrModule::parse(&text);
            let roots: Vec<&str> = module
                .functions()
                .iter()
                .filter(|function| pattern.is_match(&function.name))
                .map(|function| function.name.as_str())
                .collect();
            let mut progressed = false;
            for callee in module.unresolved(&roots, self.depth) {
                let Some(&offset) = symbols.get(&callee) else {
                    continue;
                };
                let member = archive.member(offset)?;
                if !loaded.contains(member.name()) {
                    load_member(&mut text, &mut loaded, member.name(), member.data(data)?, staging)?;
                    progressed = true;
                }
            }
            if !progressed {
                return Ok(text);
            }
        }
    }
}

fn load_member(
    text: &mut String,
    loaded: &mut HashSet<Vec<u8>>,
    name: &[u8],
    bytes: &[u8],
    staging: &Path,
) -> Result<(), Error> {
    let display = String::from_utf8_lossy(name).into_owned();
    eprintln!("disassembling archive member {display}");
    let path = staging.join(format!("{}.air", loaded.len()));
    fs::write(&path, bytes).map_err(|source| Error::Write {
        path: path.clone(),
        source,
    })?;
    text.push_str(&format!("; archive member {display}\n"));
    text.push_str(&objdump(&path)?);
    loaded.insert(name.to_vec());
    Ok(())
}

fn objdump(path: &Path) -> Result<String, Error> {
    let output = Command::new("xcrun").args(["metal-objdump", "-d"]).arg(path).output().map_err(Error::Spawn)?;
    if !output.status.success() {
        return Err(Error::Disassembly {
            path: path.to_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn print_counts(
    totals: HashMap<String, usize>,
    top: usize,
    indent: usize,
) {
    let mut totals: Vec<_> = totals.into_iter().collect();
    totals.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    for (name, count) in totals.iter().take(top) {
        println!("{:indent$}{count:7} {name}", "");
    }
}
