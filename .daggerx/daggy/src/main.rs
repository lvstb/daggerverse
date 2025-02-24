use std::env;
use std::fs::{self};
use std::io::{Write, Error, ErrorKind};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use clap::Parser;
use serde::Deserialize;
use regex::Regex;
use serde_json::{json, Value};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Task is the name of the task to run
    #[arg(short = 't', long = "task")]
    task: String,

    /// Module is the name of the dagger module to generate.
    #[arg(short = 'm', long = "module")]
    module: Option<String>,
}

#[derive(Deserialize)]
struct NewDaggerModule {
    path: String,
    name: String,
    module_src_path: String,
    module_test_src_path: String,
    github_actions_workflow_path: String,
    github_actions_workflow: String,
}
fn main() -> Result<(), Error> {
    let args: Args = Args::parse();

    match args.task.as_str() {
        "create" => {
            if let Some(module) = args.module {
                create_module(&module)?;
            } else {
                eprintln!("Module name is required for 'create' task");
                std::process::exit(1);
            }
        },
        "develop" => {
            develop_modules()?;
        },
        _ => {
            eprintln!("Unknown task: {}", args.task);
            std::process::exit(1);
        }
    }

    Ok(())
}

// Create a new module in the root of the current directory.
fn create_module(module: &str) -> Result<(), Error> {
    println!("Creating module 🚀: {}", module);
    dagger_module_exists(module)?;

    let git_root = get_git_root()?;
    env::set_current_dir(git_root)?;

    let new_module = get_module_configurations(module)?;
    println!("Module path: {}", new_module.path);
    println!("Module src path: {}", new_module.module_src_path);
    println!("Module test src path: {}", new_module.module_test_src_path);
    println!("GitHub Actions workflow path: {}", new_module.github_actions_workflow_path);

    // Initialize the new module
    initialize_module(&new_module)?;

    // Initialize examples and tests
    initialize_examples(&new_module)?;
    initialize_tests(&new_module)?;

    // Copy README and LICENSE files
    copy_readme_and_license(&new_module)?;

    // Update README content
    update_readme_content(&new_module)?;

    // Generate GitHub Actions workflow
    generate_github_actions_workflow(&new_module)?;

    // Run go fmt to format the code
    println!("Running go fmt and ensuring the code is formatted correctly 🧹");
    run_go_fmt(&new_module.path)?;
    run_go_fmt(&format!("{}/examples/go", new_module.path))?;
    run_go_fmt(&new_module.module_test_src_path)?;

    println!("Module \"{}\" initialized successfully 🎉", new_module.name);
    println!("Don't forget to add it to GitHub Actions workflow 'release.yml' when your module is ready for release.");
    println!("It's recommended to run just cilocal <newmodule> to test the module locally before releasing it.");

    Ok(())
}

fn copy_and_process_templates(module_cfg: &NewDaggerModule, template_dir: &str, dest_dir: &str) -> Result<(), Error> {
    for entry in fs::read_dir(template_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let new_dir = format!("{}/{}", dest_dir, entry.file_name().to_string_lossy());
            fs::create_dir_all(&new_dir)?;
            copy_and_process_templates(module_cfg, &path.to_string_lossy(), &new_dir)?;
        } else {
            let content = fs::read_to_string(&path)?;
            let new_content = process_template_content(&content, module_cfg);

            let dest_file_name = entry.file_name().to_string_lossy().replace(".tmpl", "");
            let dest_path = format!("{}/{}", dest_dir, dest_file_name);
            fs::write(dest_path, new_content)?;
        }
    }

    Ok(())
}

// New function
// Modified function
fn copy_dir_recursive(src: &Path, dest: &Path, module_cfg: &NewDaggerModule) -> Result<(), Error> {
    if !dest.exists() {
        fs::create_dir_all(dest)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let mut file_name = entry.file_name().to_string_lossy().to_string();

        // Remove .tmpl extension if present
        if file_name.ends_with(".tmpl") {
            file_name = file_name.trim_end_matches(".tmpl").to_string();
        }

        let dest_path = dest.join(&file_name);

        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dest_path, module_cfg)?;
        } else {
            let content = fs::read_to_string(&src_path)?;
            let processed_content = process_template_content(&content, module_cfg);
            fs::write(dest_path, processed_content)?;
        }
    }

    Ok(())
}

// New function
fn process_template_content(content: &str, module_cfg: &NewDaggerModule) -> String {
    // let pkg_name = module_cfg.name.replace("-", "_");
    let pkg_name = module_cfg.name.to_string().to_lowercase().trim().replace(" ", "-");
    let pascal_case_name = to_pascal_case(&module_cfg.name);
    let lowercase_name = module_cfg.name.to_lowercase();

    let content = content.replace("{{.module_name_pkg}}", &pkg_name);
    let content = content.replace("{{.module_name}}", &pascal_case_name);
    content.replace("{{.module_name_lowercase}}", &lowercase_name)
}

fn develop_modules() -> Result<(), Error> {
    // Ensure we're in a Git repository
    if !Path::new(".git").exists() {
        return Err(Error::new(ErrorKind::NotFound, "Error: This script must be run from the root of a Git repository."));
    }

    println!("Git repository detected. Proceeding...");

    // Find all directories containing a 'dagger.json' file
    let modules = find_dagger_modules()?;

    if modules.is_empty() {
        println!("No modules found.");
        return Ok(());
    }

    // Initialize counters
    let total_modules = modules.len();
    let mut successful_modules = 0;
    let mut failed_modules = 0;

    println!("Identifying modules with dagger.json files...");
    for dir in &modules {
        println!("Module identified: {}", dir);
    }

    println!("\nRunning dagger develop in identified modules...\n");

    for dir in &modules {
        print!("Developing module: {}... ", dir);

        if Path::new(&format!("{}/dagger.json", dir)).exists() {
            println!("Entering directory: {}", dir);
            match run_dagger_develop(dir) {
                Ok(_) => {
                    println!("✅ Successfully developed module: {}", dir);
                    successful_modules += 1;
                }
                Err(e) => {
                    println!("❌ Failed to develop module: {}", dir);
                    eprintln!("Error: {}", e);
                    failed_modules += 1;
                }
            }
        } else {
            println!("Skipped 🚫 No dagger.json found in: {}", dir);
        }
    }

    println!("\n");

    if successful_modules == total_modules {
        println!("Dagger develop completed for all {} modules successfully! 🎉", total_modules);
    } else if failed_modules > 0 {
        println!("Dagger develop completed with {} successes ✅ and {} failures ❌.", successful_modules, failed_modules);
        return Err(Error::new(ErrorKind::Other, "Some modules failed to develop"));
    } else {
        println!("Dagger develop completed with {} successes ✅. Please check the output above.", successful_modules);
    }

    Ok(())
}

fn update_dagger_json(module_cfg: &NewDaggerModule) -> Result<(), Error> {
    let dagger_json_path = format!("{}/dagger.json", module_cfg.path);

    let mut json_content: Value = fs::read_to_string(&dagger_json_path)
        .map_err(|e| Error::new(ErrorKind::Other, format!("Failed to read dagger.json: {}", e)))
        .and_then(|content| serde_json::from_str(&content)
            .map_err(|e| Error::new(ErrorKind::Other, format!("Failed to parse dagger.json: {}", e))))?;

    json_content["exclude"] = json!([
        "../.direnv",
        "../.devenv",
        "../go.work",
        "../go.work.sum",
        "tests",
        "examples/go",
    ]);

    fs::write(dagger_json_path, serde_json::to_string_pretty(&json_content)?)
        .map_err(|e| Error::new(ErrorKind::Other, format!("Failed to write updated dagger.json: {}", e)))?;

    Ok(())
}

// New function
fn update_tests_dagger_json(module_cfg: &NewDaggerModule) -> Result<(), Error> {
    let dagger_json_path = format!("{}/tests/dagger.json", module_cfg.path);
    let mut json_content: Value = fs::read_to_string(&dagger_json_path)
        .map_err(|e| Error::new(ErrorKind::Other, format!("Failed to read tests/dagger.json: {}", e)))
        .and_then(|content| serde_json::from_str(&content)
            .map_err(|e| Error::new(ErrorKind::Other, format!("Failed to parse tests/dagger.json: {}", e))))?;

    json_content["exclude"] = json!([
        "../../.direnv",
        "../../.devenv",
        "../../go.work",
        "../../go.work.sum"
    ]);

    fs::write(dagger_json_path, serde_json::to_string_pretty(&json_content)?)
        .map_err(|e| Error::new(ErrorKind::Other, format!("Failed to write updated tests/dagger.json: {}", e)))?;

    Ok(())
}

// New function
fn update_examples_dagger_json(module_cfg: &NewDaggerModule) -> Result<(), Error> {
    let dagger_json_path = format!("{}/examples/go/dagger.json", module_cfg.path);
    let mut json_content: Value = fs::read_to_string(&dagger_json_path)
        .map_err(|e| Error::new(ErrorKind::Other, format!("Failed to read examples/go/dagger.json: {}", e)))
        .and_then(|content| serde_json::from_str(&content)
            .map_err(|e| Error::new(ErrorKind::Other, format!("Failed to parse examples/go/dagger.json: {}", e))))?;

    json_content["exclude"] = json!([
        "../../../.direnv",
        "../../../.devenv",
        "../../../go.work",
        "../../../go.work.sum"
    ]);

    fs::write(dagger_json_path, serde_json::to_string_pretty(&json_content)?)
        .map_err(|e| Error::new(ErrorKind::Other, format!("Failed to write updated examples/go/dagger.json: {}", e)))?;

    Ok(())
}

fn initialize_module(module_cfg: &NewDaggerModule) -> Result<(), Error> {
    // Create the module directory
    fs::create_dir_all(&module_cfg.path)?;
    println!("Creating parent module 📦: {}", module_cfg.name);

    // Change to the module directory
    env::set_current_dir(&module_cfg.path)?;

    // Run dagger init
    run_command_with_output(&format!("dagger init --sdk go --name {} --source .", module_cfg.name), ".")?;

    // Copy and process templates
    copy_and_process_templates(module_cfg, "../.daggerx/templates/module", ".")?;

    // Update dagger.json
    update_dagger_json(module_cfg)?;

    // Edit go.mod to set the correct module path
    let go_mod_edit_command = format!("go mod edit -module github.com/Excoriate/daggerverse/{}", module_cfg.name);
    run_command_with_output(&go_mod_edit_command, ".")?;

    // Run dagger develop
    run_command_with_output(&format!("dagger develop -m {}", module_cfg.name), ".")?;

    // Change back to the root directory
    env::set_current_dir("..")?;

    Ok(())
}

// New function
// Modified function
fn initialize_examples(module_cfg: &NewDaggerModule) -> Result<(), Error> {
    let examples_path = format!("{}/examples/go", module_cfg.path);
    println!("Creating examples module (recipes)  📄: {}", module_cfg.name);

    // Create the examples directory
    fs::create_dir_all(&examples_path)?;

    // Change to the examples directory
    env::set_current_dir(&examples_path)?;

    // Run dagger init
    run_command_with_output("dagger init --sdk go --name go --source .", ".")?;

    // Copy and process templates
    copy_and_process_templates(module_cfg, "../../../.daggerx/templates/examples/go", ".")?;

    // Update dagger.json
    update_examples_dagger_json(module_cfg)?;

    // Edit go.mod
    let go_mod_edit_command = format!("go mod edit -module github.com/Excoriate/daggerverse/{}/examples/go", module_cfg.name);
    run_command_with_output(&go_mod_edit_command, ".")?;

    // Copy testdata/common directory
    let src_testdata = "../../../.daggerx/templates/examples/go/testdata/common";
    let dest_testdata = "testdata/common";
    copy_dir_all(src_testdata, dest_testdata)?;

    // Run dagger install and develop
    run_command_with_output("dagger install ../../", ".")?;
    run_command_with_output("dagger develop -m go", ".")?;

    // Change back to the root directory
    env::set_current_dir("../../..")?;

    Ok(())
}

// New helper function to copy directories recursively
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<(), Error> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

// Modified function
fn initialize_tests(module_cfg: &NewDaggerModule) -> Result<(), Error> {
    let tests_path = format!("{}/tests", module_cfg.path);
    println!("Creating tests module (tests) 🧪: {}", module_cfg.name);

    // Create the tests directory
    fs::create_dir_all(&tests_path)?;

    // Change to the tests directory
    env::set_current_dir(&tests_path)?;

    // Run dagger init
    run_command_with_output("dagger init --sdk go --name tests --source .", ".")?;

    // Copy and process templates
    copy_and_process_templates(module_cfg, "../../.daggerx/templates/tests", ".")?;

    // Update dagger.json
    update_tests_dagger_json(module_cfg)?;

    // Edit go.mod
    let go_mod_edit_command = format!("go mod edit -module github.com/Excoriate/daggerverse/{}/tests", module_cfg.name);
    run_command_with_output(&go_mod_edit_command, ".")?;

    // Copy testdata/common directory
    let src_testdata = "../../.daggerx/templates/tests/testdata/common";
    let dest_testdata = "testdata/common";
    copy_dir_all(src_testdata, dest_testdata)?;

    // Run dagger install and develop
    run_command_with_output("dagger install ../", ".")?;
    run_command_with_output("dagger develop -m tests", ".")?;

    // Change back to the root directory
    env::set_current_dir("../..")?;

    Ok(())
}

fn get_git_root() -> Result<String, Error> {
    let output = Command::new("git")
        .args(&["rev-parse", "--show-toplevel"])
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(Error::new(ErrorKind::Other, "Not in a git repository"))
    }
}

fn copy_and_replace_templates(template_dir: &str, destination_dir: &str, module_name: &str) -> Result<(), Error> {
    for entry in fs::read_dir(template_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let new_dir = format!("{}/{}", destination_dir, entry.file_name().to_string_lossy());
            fs::create_dir_all(&new_dir)?;
            copy_and_replace_templates(&path.to_string_lossy(), &new_dir, module_name)?;
        } else {
            let content = fs::read_to_string(&path)?;
            let new_content = if path.extension().map_or(false, |ext| ext == "go") {
                replace_module_name(&content, &to_pascal_case(&module_name))
            } else {
                replace_module_name(&content, module_name)
            };

            let dest_file_name = entry.file_name().to_string_lossy().replace(".tmpl", "");
            let dest_path = format!("{}/{}", destination_dir, dest_file_name);
            fs::write(dest_path, new_content)?;
        }
    }

    Ok(())
}

fn copy_readme_and_license(module_cfg: &NewDaggerModule) -> Result<(), Error> {
    let readme_dest_path = format!("{}/README.md", module_cfg.path);
    let license_dest_path = format!("{}/LICENSE", module_cfg.path);
    println!("Copying README.md and LICENSE files 📄: {}", module_cfg.name);

    // Ensure the destination directory exists
    fs::create_dir_all(&module_cfg.path)?;

    // Copy the README.md and LICENSE files from the template directory to the module path
    fs::copy(".daggerx/templates/README.md", &readme_dest_path)?;
    fs::copy(".daggerx/templates/LICENSE", &license_dest_path)?;

    // Replace placeholders in README.md if any
    let readme_content = fs::read_to_string(&readme_dest_path)?;
    let new_readme_content = readme_content.replace("[@MODULE_NAME]", &module_cfg.name);
    fs::write(readme_dest_path, new_readme_content)?;

    Ok(())
}


// Modified function
fn get_module_configurations(module: &str) -> Result<NewDaggerModule, Error> {
    let module_path_full = env::current_dir()?.join(module);
    let current_root_dir = env::current_dir()?;

    Ok(NewDaggerModule {
        path: module_path_full.to_string_lossy().to_string(),
        module_src_path: module_path_full.to_string_lossy().to_string(),
        module_test_src_path: module_path_full.join("tests").to_string_lossy().to_string(),
        name: module.to_string(),
        github_actions_workflow_path: current_root_dir.join(".github/workflows").to_string_lossy().to_string(),
        github_actions_workflow: current_root_dir.join(".github/workflows").join(format!("ci-mod-{}.yaml", module)).to_string_lossy().to_string(),
    })
}

// Modified function
fn generate_github_actions_workflow(module_cfg: &NewDaggerModule) -> Result<(), Error> {
    println!("Generating GitHub Actions workflow 🚀: {}", module_cfg.name);
    fs::create_dir_all(&module_cfg.github_actions_workflow_path)?;
    let template_path = ".daggerx/templates/github/workflows/mod-template-ci.yaml.tmpl";
    let output_path = &module_cfg.github_actions_workflow;

    let template_content = fs::read_to_string(template_path)?;
    let new_content = process_template_content(&template_content, module_cfg);
    fs::write(output_path, new_content)?;

    Ok(())
}

fn dagger_module_exists(module: &str) -> Result<(), Error> {
    if module.is_empty() {
        return Err(Error::new(ErrorKind::InvalidInput, "Module name cannot be empty"));
    }

    // Check if the module already exists in the root of this directory.
    if Path::new(module).exists() {
        return Err(Error::new(ErrorKind::AlreadyExists, "Module already exists"));
    }

    Ok(())
}

fn find_git_root() -> Result<String, Error> {
    let output = Command::new("git")
        .args(&["rev-parse", "--show-toplevel"])
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(Error::new(ErrorKind::Other, "Not in a git repository"))
    }
}
fn run_command_with_output(command: &str, target_dir: &str) -> Result<Output, Error> {
    println!("Running command: {}", command);
    let target_directory = if target_dir.is_empty() { find_git_root()? } else { target_dir.to_string() };

    println!("Running command in directory: {}", target_directory);
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(target_directory)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()?;

    if !output.status.success() {
        return Err(Error::new(ErrorKind::Other, format!("Command failed with exit code: {} and with error: {}", output.status, String::from_utf8_lossy(&output.stderr))));
    }

    Ok(output)
}

fn replace_module_name(content: &str, module_name: &str) -> String {
    let pascal_case_name = to_pascal_case(module_name);
    let camel_case_name = to_camel_case(module_name);

    let re_pascal = Regex::new(r"\{\{\s*\.\s*module_name\s*\}\}").unwrap();
    let re_camel = Regex::new(r"\{\{\s*\.\s*module_name_camel\s*\}\}").unwrap();
    let re_lowercase = Regex::new(r"\{\{\s*\.\s*module_name_lowercase\s*\}\}").unwrap();

    let content = re_pascal.replace_all(content, &pascal_case_name);
    let content = re_camel.replace_all(&content, &camel_case_name);
    re_lowercase.replace_all(&content, module_name).to_string()
}

fn capitalize_module_name(module_name: &str) -> String {
    let mut chars = module_name.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}


fn update_readme_content(module_cfg: &NewDaggerModule) -> Result<(), Error> {
    let readme_path = format!("{}/README.md", module_cfg.path);
    println!("Updating README.md content 📄: {}", module_cfg.name);

    if !Path::new(&readme_path).exists() {
        return Err(Error::new(ErrorKind::NotFound, format!("README.md file not found in {}", module_cfg.path)));
    }

    let readme_content = fs::read_to_string(&readme_path)?;
    let new_content = replace_module_name_smart(&readme_content, &module_cfg.name);
    fs::write(&readme_path, new_content)?;

    Ok(())
}

fn replace_module_name_smart(content: &str, module_name: &str) -> String {
    let pascal_case_name = to_pascal_case(module_name);
    let lowercase_name = module_name.to_lowercase();

    let re = Regex::new(r"```[\s\S]*?```|`[^`\n]+`|\{\{\s*\.\s*module_name\s*\}\}").unwrap();

    re.replace_all(content, |caps: &regex::Captures| {
        let matched = caps.get(0).unwrap().as_str();
        if matched.starts_with("```") || matched.starts_with("`") {
            // Inside code blocks, use lowercase with hyphens
            matched.replace("{{.module_name}}", &lowercase_name)
        } else {
            // Outside code blocks, use PascalCase without hyphens
            matched.replace("{{.module_name}}", &pascal_case_name)
        }
    }).to_string()
}

fn to_camel_case(s: &str) -> String {
    s.split('-')
        .enumerate()
        .map(|(i, part)| {
            if i == 0 {
                part.to_lowercase()
            } else {
                capitalize_module_name(part)
            }
        })
        .collect()
}

fn to_pascal_case(s: &str) -> String {
    s.split('-')
        .map(capitalize_module_name)
        .collect()
}

fn run_go_fmt(module_path: &str) -> Result<(), Error> {
    run_command_with_output("go fmt ./...", module_path)?;
    Ok(())
}

fn find_dagger_modules() -> Result<Vec<String>, Error> {
    let output = Command::new("find")
        .args(&[".", "-type", "f", "-name", "dagger.json"])
        .output()?;

    if !output.status.success() {
        return Err(Error::new(ErrorKind::Other, "Failed to execute find command"));
    }

    let modules = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| Path::new(line).parent().unwrap().to_string_lossy().into_owned())
        .collect::<Vec<String>>();

    Ok(modules)
}

fn run_dagger_develop(dir: &str) -> Result<(), Error> {
    let output = Command::new("dagger")
        .arg("develop")
        .current_dir(dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()?;

    if !output.status.success() {
        return Err(Error::new(ErrorKind::Other, format!("dagger develop failed in directory: {}", dir)));
    }

    Ok(())
}
