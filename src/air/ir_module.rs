use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::LazyLock,
};

use regex::Regex;

use super::ir_function::IrFunction;

static DEFINITION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^define\b[^@]*@("(?:[^"\\]|\\.)*"|[-\w.$]+)"#).expect("valid definition pattern"));
static CALL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?:^|\s)call\b[^@]*@("(?:[^"\\]|\\.)*"|[-\w.$]+)"#).expect("valid call pattern"));

#[derive(Default)]
pub struct IrModule {
    functions: Vec<IrFunction>,
    index: HashMap<String, usize>,
}

impl IrModule {
    pub fn parse(text: &str) -> Self {
        let mut module = Self::default();
        let mut current: Option<IrFunction> = None;
        for line in text.lines() {
            if let Some(definition) = DEFINITION.captures(line) {
                current = Some(IrFunction {
                    name: definition[1].trim_matches('"').to_owned(),
                    ..IrFunction::default()
                });
                continue;
            }
            let Some(function) = current.as_mut() else {
                continue;
            };
            if line.starts_with('}') {
                if let Some(function) = current.take()
                    && !module.index.contains_key(&function.name)
                {
                    module.index.insert(function.name.clone(), module.functions.len());
                    module.functions.push(function);
                }
                continue;
            }
            let body = line.trim();
            if body.is_empty()
                || body.starts_with(';')
                || body.split_whitespace().next().is_some_and(|word| word.ends_with(':'))
            {
                continue;
            }
            function.instructions += 1;
            if let Some(call) = CALL.captures(body) {
                function.add_call(call[1].trim_matches('"'));
            }
        }
        module
    }

    pub fn functions(&self) -> &[IrFunction] {
        &self.functions
    }

    pub fn get(
        &self,
        name: &str,
    ) -> Option<&IrFunction> {
        self.index.get(name).map(|&index| &self.functions[index])
    }

    pub fn unresolved(
        &self,
        roots: &[&str],
        depth: usize,
    ) -> BTreeSet<String> {
        let mut frontier: Vec<(&str, usize)> = roots.iter().map(|&root| (root, depth)).collect();
        let mut seen = HashSet::new();
        let mut missing = BTreeSet::new();
        while let Some((name, remaining)) = frontier.pop() {
            let Some(function) = self.get(name).filter(|_| seen.insert(name)) else {
                continue;
            };
            for (callee, _) in function.calls.iter().filter(|(callee, _)| !is_intrinsic(callee)) {
                match self.get(callee) {
                    Some(_) if remaining > 0 => frontier.push((callee, remaining - 1)),
                    None if remaining > 0 => {
                        missing.insert(callee.clone());
                    },
                    _ => {},
                }
            }
        }
        missing
    }

    pub fn print_calls(
        &self,
        name: &str,
        weight: usize,
        depth: usize,
        stack: &mut Vec<String>,
        intrinsics: &mut HashMap<String, usize>,
    ) {
        let Some(function) = self.get(name) else {
            return;
        };
        let prefix = " ".repeat(2 + 7 * stack.len());
        stack.push(name.to_owned());
        for (callee, count) in &function.calls {
            if callee.starts_with("air.") {
                *intrinsics.entry(callee.clone()).or_default() += count * weight;
            }
            match self.get(callee) {
                Some(child) if depth > 0 && !stack.contains(callee) => {
                    println!("{prefix}{count:>5}x {callee} ({} instructions)", child.instructions);
                    self.print_calls(callee, weight * count, depth - 1, stack, intrinsics);
                },
                Some(child) => println!("{prefix}{count:>5}x {callee} (defined, {} instructions)", child.instructions),
                None => println!("{prefix}{count:>5}x {callee}"),
            }
        }
        stack.pop();
    }
}

fn is_intrinsic(name: &str) -> bool {
    name.starts_with("air.") || name.starts_with("llvm.")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::IrModule;

    const IR: &str = r#"
define internal fastcc void @"helper.1"(<8 x i8> %0) #1 {
  %2 = tail call <8 x i32> @air.simdgroup_matrix_16x16x16_widening_multiply_accumulate.s.s.v8i32.v8i8.v8i8.v8i32(<8 x i8> %0)
  %3 = tail call <8 x i32> @air.simdgroup_matrix_16x16x16_widening_multiply_accumulate.s.s.v8i32.v8i8.v8i8.v8i32(<8 x i8> %0)
  ret void
}

; Function Attrs: convergent
define void @kernel(i8 addrspace(1)* %0) local_unnamed_addr #0 {
  br label %2

2:                                                ; preds = %1
  call fastcc void @"helper.1"(<8 x i8> zeroinitializer)
  call fastcc void @"helper.1"(<8 x i8> zeroinitializer)
  call void @air.wg.barrier(i32 2, i32 1)
  ret void
}
"#;

    #[test]
    fn parses_definitions_and_calls() {
        let module = IrModule::parse(IR);
        assert_eq!(module.functions().len(), 2);
        let kernel = module.get("kernel").expect("kernel is defined");
        assert_eq!(kernel.instructions, 5);
        assert_eq!(kernel.calls, [("helper.1".to_owned(), 2), ("air.wg.barrier".to_owned(), 1)]);
        let mut totals = HashMap::new();
        module.print_calls("kernel", 1, 1, &mut Vec::new(), &mut totals);
        assert_eq!(totals["air.simdgroup_matrix_16x16x16_widening_multiply_accumulate.s.s.v8i32.v8i8.v8i8.v8i32"], 4);
        assert_eq!(totals["air.wg.barrier"], 1);
        assert!(module.unresolved(&["kernel"], 1).is_empty());
        let caller = IrModule::parse("define void @root() {\n  call void @external()\n  call void @llvm.foo()\n}\n");
        assert_eq!(caller.unresolved(&["root"], 1).into_iter().collect::<Vec<_>>(), ["external"]);
        assert!(caller.unresolved(&["root"], 0).is_empty());
    }
}
