use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    time::Duration,
};

use bootloader_locator::{CargoMetadataError, LocateError};
use locate_cargo_manifest::LocateManifestError;

const RUN_ARGS: &[&str] = &[
    "--no-reboot",
    "-s",
    //"-S",
    "-serial",
    "mon:stdio",
    "-d",
    "int",
    "-M",
    "smm=off",
    "-D",
    "qemu_debug.log",
];
const TEST_ARGS: &[&str] = &[
    "-device",
    "isa-debug-exit,iobase=0xf4,iosize=0x04",
    "-serial",
    "stdio",
    "-display",
    "none",
    "--no-reboot",
];
const TEST_TIMEOUT_SECS: u64 = 10;

fn main() {
    let mut args = std::env::args().skip(1); // skip executable name

    let kernel_binary_path = {
        let path = PathBuf::from_iter(["../", args.next().as_ref().unwrap()]);
        path.canonicalize().unwrap()
    };
    let no_boot = if let Some(arg) = args.next() {
        match arg.as_str() {
            "--no-run" => true,
            other => panic!("unexpected argument `{}`", other),
        }
    } else {
        false
    };

    let binary_kind = runner_utils::binary_kind(&kernel_binary_path);
    let bios = create_disk_images(&kernel_binary_path, binary_kind.is_test());

    if no_boot {
        println!("Created disk image at `{}`", bios.display());
        return;
    }

    let mut run_cmd = Command::new("qemu-system-x86_64");
    run_cmd
        .arg("-drive")
        .arg(format!("format=raw,file={}", bios.display()));

    if binary_kind.is_test() {
        run_cmd.args(TEST_ARGS);

        let exit_status = run_test_command(run_cmd);
        match exit_status.code() {
            Some(33) => {} // success
            other => panic!("Test failed (exit code: {:?})", other),
        }
    } else {
        run_cmd.args(RUN_ARGS);

        let exit_status = run_cmd.status().unwrap();
        if !exit_status.success() {
            std::process::exit(exit_status.code().unwrap_or(1));
        }
    }
}

fn run_test_command(mut cmd: Command) -> ExitStatus {
    runner_utils::run_with_timeout(&mut cmd, Duration::from_secs(TEST_TIMEOUT_SECS)).unwrap()
}

fn metadata() -> Result<json::JsonValue, CargoMetadataError> {
    let parent_path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR")))
        .parent()
        .unwrap()
        .to_path_buf();
    let mut cmd = Command::new(env!("CARGO"));
    cmd.current_dir(parent_path);
    cmd.arg("metadata");
    cmd.arg("--format-version").arg("1");
    let output = cmd.output()?;

    if !output.status.success() {
        return Err(CargoMetadataError::Failed {
            stderr: output.stderr,
        });
    }

    let output = String::from_utf8(output.stdout)?;
    let parsed = json::parse(&output)?;

    Ok(parsed)
}

pub fn locate_bootloader(dependency_name: &str) -> Result<PathBuf, LocateError> {
    let metadata = metadata()?;

    let root = metadata["resolve"]["root"]
        .as_str()
        .ok_or(LocateError::MetadataInvalid)?;

    let root_resolve = metadata["resolve"]["nodes"]
        .members()
        .find(|r| r["id"] == root)
        .ok_or(LocateError::MetadataInvalid)?;

    let dependency = root_resolve["deps"]
        .members()
        .find(|d| d["name"] == dependency_name)
        .ok_or(LocateError::DependencyNotFound)?;
    let dependency_id = dependency["pkg"]
        .as_str()
        .ok_or(LocateError::MetadataInvalid)?;

    let dependency_package = metadata["packages"]
        .members()
        .find(|p| p["id"] == dependency_id)
        .ok_or(LocateError::MetadataInvalid)?;
    let dependency_manifest = dependency_package["manifest_path"]
        .as_str()
        .ok_or(LocateError::MetadataInvalid)?;

    Ok(dependency_manifest.into())
}

pub fn locate_manifest() -> Result<PathBuf, LocateManifestError> {
    let parent_path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR")))
        .parent()
        .unwrap()
        .to_path_buf();
    let cargo = env::var("CARGO").unwrap_or("cargo".to_owned());
    let output = Command::new(cargo)
        .current_dir(parent_path)
        .arg("locate-project")
        .output()?;
    if !output.status.success() {
        return Err(LocateManifestError::CargoExecution {
            stderr: output.stderr,
        });
    }

    let output = String::from_utf8(output.stdout)?;
    let parsed = json::parse(&output)?;
    let root = parsed["root"].as_str().ok_or(LocateManifestError::NoRoot)?;
    Ok(PathBuf::from(root))
}

pub fn create_disk_images(kernel_binary_path: &Path, is_test: bool) -> PathBuf {
    let bootloader_manifest_path = locate_bootloader("bootloader").unwrap();
    let kernel_manifest_path = locate_manifest().unwrap();

    let mut build_cmd = Command::new(env!("CARGO"));
    build_cmd.current_dir(bootloader_manifest_path.parent().unwrap());
    build_cmd.arg("builder");
    build_cmd
        .arg("--kernel-manifest")
        .arg(&kernel_manifest_path);
    build_cmd.arg("--kernel-binary").arg(&kernel_binary_path);
    build_cmd
        .arg("--target-dir")
        .arg(kernel_manifest_path.parent().unwrap().join("target"));
    build_cmd
        .arg("--out-dir")
        .arg(kernel_binary_path.parent().unwrap());
    if is_test {
        build_cmd.arg("--quiet");
    }

    if !build_cmd.status().unwrap().success() {
        panic!("build failed");
    }

    let kernel_binary_name = kernel_binary_path.file_name().unwrap().to_str().unwrap();
    let disk_image = kernel_binary_path
        .parent()
        .unwrap()
        .join(format!("boot-bios-{}.img", kernel_binary_name));
    if !disk_image.exists() {
        panic!(
            "Disk image does not exist at {} after bootloader build",
            disk_image.display()
        );
    }
    disk_image
}
