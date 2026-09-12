//! Benchmarks for scanner-core parsing performance.
//!
//! Run with: cargo bench --bench parser_bench

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::io::Write;
use tempfile::NamedTempFile;

use scanner_core::{parse_file, Language, SymbolIndex};

/// Performance budget constants (milliseconds per 1000 lines)
const BUDGET_MS_PER_1000_LINES: u64 = 50;

/// Generate Python code with n classes and n methods each.
fn generate_python_code(num_classes: usize, methods_per_class: usize) -> String {
    let mut code = String::new();
    code.push_str("import os\nfrom collections import defaultdict\n\n");

    for i in 0..num_classes {
        code.push_str(&format!("class Class{i}(BaseClass):\n"));
        code.push_str(&format!("    \"\"\"Docstring for Class{i}\"\"\"\n\n"));

        for j in 0..methods_per_class {
            code.push_str(&format!("    @decorator\n"));
            code.push_str(&format!("    def method_{j}(self, arg1, arg2):\n"));
            code.push_str(&format!("        \"\"\"Method docstring\"\"\"\n"));
            code.push_str(&format!("        result = self.helper_{j}()\n"));
            code.push_str(&format!("        return result + {j}\n\n"));
        }
    }

    code
}

/// Generate JavaScript code with n classes.
fn generate_javascript_code(num_classes: usize, methods_per_class: usize) -> String {
    let mut code = String::new();
    code.push_str("import { Base } from './base';\n\n");

    for i in 0..num_classes {
        code.push_str(&format!("class Class{i} extends Base {{\n"));

        for j in 0..methods_per_class {
            code.push_str(&format!("    method_{j}(arg1, arg2) {{\n"));
            code.push_str(&format!("        const result = this.helper_{j}();\n"));
            code.push_str(&format!("        return result + {j};\n"));
            code.push_str(&format!("    }}\n\n"));
        }

        code.push_str("}\n\n");
    }

    code
}

/// Generate Rust code with n structs and impls.
fn generate_rust_code(num_structs: usize, methods_per_struct: usize) -> String {
    let mut code = String::new();
    code.push_str("use std::collections::HashMap;\n\n");

    for i in 0..num_structs {
        code.push_str(&format!("pub struct Struct{i} {{\n"));
        code.push_str(&format!("    field1: i32,\n"));
        code.push_str(&format!("    field2: String,\n"));
        code.push_str(&format!("}}\n\n"));

        code.push_str(&format!("impl Struct{i} {{\n"));

        for j in 0..methods_per_struct {
            code.push_str(&format!("    pub fn method_{j}(&self, arg: i32) -> i32 {{\n"));
            code.push_str(&format!("        self.field1 + arg + {j}\n"));
            code.push_str(&format!("    }}\n\n"));
        }

        code.push_str("}\n\n");
    }

    code
}

fn bench_python_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("python_parsing");

    for size in [10, 50, 100].iter() {
        let code = generate_python_code(*size, 5);
        let line_count = code.lines().count();

        group.throughput(Throughput::Elements(line_count as u64));
        group.bench_with_input(
            BenchmarkId::new("classes", size),
            &code,
            |b, code| {
                b.iter(|| {
                    let mut file = NamedTempFile::new().unwrap();
                    file.write_all(code.as_bytes()).unwrap();

                    let mut index = SymbolIndex::new();
                    let path = file.path();
                    let _ = parse_file(
                        black_box(path),
                        black_box(path),
                        black_box(Language::Python),
                        black_box(&mut index),
                    );
                });
            },
        );
    }

    group.finish();
}

fn bench_javascript_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("javascript_parsing");

    for size in [10, 50, 100].iter() {
        let code = generate_javascript_code(*size, 5);
        let line_count = code.lines().count();

        group.throughput(Throughput::Elements(line_count as u64));
        group.bench_with_input(
            BenchmarkId::new("classes", size),
            &code,
            |b, code| {
                b.iter(|| {
                    let mut file = NamedTempFile::new().unwrap();
                    file.write_all(code.as_bytes()).unwrap();

                    let mut index = SymbolIndex::new();
                    let path = file.path();
                    let _ = parse_file(
                        black_box(path),
                        black_box(path),
                        black_box(Language::JavaScript),
                        black_box(&mut index),
                    );
                });
            },
        );
    }

    group.finish();
}

fn bench_rust_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("rust_parsing");

    for size in [10, 50, 100].iter() {
        let code = generate_rust_code(*size, 5);
        let line_count = code.lines().count();

        group.throughput(Throughput::Elements(line_count as u64));
        group.bench_with_input(
            BenchmarkId::new("structs", size),
            &code,
            |b, code| {
                b.iter(|| {
                    let mut file = NamedTempFile::new().unwrap();
                    file.write_all(code.as_bytes()).unwrap();

                    let mut index = SymbolIndex::new();
                    let path = file.path();
                    let _ = parse_file(
                        black_box(path),
                        black_box(path),
                        black_box(Language::Rust),
                        black_box(&mut index),
                    );
                });
            },
        );
    }

    group.finish();
}

fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload");

    // Simulate parsing a typical project with mixed languages
    let python_code = generate_python_code(20, 5);
    let js_code = generate_javascript_code(20, 5);
    let rust_code = generate_rust_code(10, 5);

    let total_lines =
        python_code.lines().count() + js_code.lines().count() + rust_code.lines().count();

    group.throughput(Throughput::Elements(total_lines as u64));

    group.bench_function("typical_project", |b| {
        b.iter(|| {
            let mut index = SymbolIndex::new();

            // Python files
            let mut py_file = NamedTempFile::new().unwrap();
            py_file.write_all(python_code.as_bytes()).unwrap();
            let _ = parse_file(
                py_file.path(),
                py_file.path(),
                Language::Python,
                &mut index,
            );

            // JS files
            let mut js_file = NamedTempFile::new().unwrap();
            js_file.write_all(js_code.as_bytes()).unwrap();
            let _ = parse_file(
                js_file.path(),
                js_file.path(),
                Language::JavaScript,
                &mut index,
            );

            // Rust files
            let mut rs_file = NamedTempFile::new().unwrap();
            rs_file.write_all(rust_code.as_bytes()).unwrap();
            let _ = parse_file(
                rs_file.path(),
                rs_file.path(),
                Language::Rust,
                &mut index,
            );

            black_box(index)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_python_parsing,
    bench_javascript_parsing,
    bench_rust_parsing,
    bench_mixed_workload,
);
criterion_main!(benches);
