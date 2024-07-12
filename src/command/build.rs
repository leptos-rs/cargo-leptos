use crate::Cli;
use color_eyre::Result;
use xshell::{Shell, cmd};
use shlex::Quoter;

pub async fn build_all(cli: &Cli) -> Result<()> {
    println!("BUILDING!");
    
    if cli.opts.lib_only && cli.opts.bin_only{
        panic!("Cannot set both lib-only and bin-only, that doesn't make sense");
    }
    else if cli.opts.lib_only{
        let _ = build_lib(cli);
    } else if cli.opts.bin_only{
        let _ = build_bin(cli);
    } else{
        let _ = build_lib(cli);
        let _ = build_bin(cli);
    }
    Ok(())
}

pub fn build_bin(cli: &Cli) -> Result<()>{
    let bin_opts = cli.opts.bin_opts.clone();
    
    let default_bin_cargo_args = vec!["build".to_string(), format!("--package={}", cli.bin_crate_name.clone().unwrap()), format!("--bin={}",cli.bin_crate_name.clone().unwrap()), "--no-default-features".to_string() ];
    let bin_cargo_args = bin_opts.bin_cargo_args.unwrap_or(default_bin_cargo_args);
    let bin_cargo_command = bin_opts.bin_cargo_command.unwrap();

    let bin_cargo_args = bin_cargo_args.join(" ");
    let bin_cargo_command = bin_cargo_command.join(" "); 

    let bin_cmd = format!("{} {}", bin_cargo_command, bin_cargo_args);

    let sh = Shell::new()?;
    Ok(cmd!(sh, "{bin_cmd}" ).run()?)

}


pub fn build_lib(cli: &Cli) -> Result<()>{

    let lib_opts = cli.opts.lib_opts.clone();
    let lib_crate_name = cli.lib_crate_name.clone().unwrap().to_string(); 
    let default_lib_cargo_args = vec!["build".to_string(), format!("--package={}",lib_crate_name), "--lib".to_string(), "--no-default-features".to_string() ];
    let lib_cargo_args = lib_opts.lib_cargo_args.unwrap_or(default_lib_cargo_args);
    let lib_cargo_command = lib_opts.lib_cargo_command.unwrap();

    let lib_cargo_args = lib_cargo_args.join(" ");
    let lib_cargo_command = lib_cargo_command.join(" "); 

    let lib_cmd = format!("{} {}", lib_cargo_command, lib_cargo_args);

    let sh = Shell::new()?;
    Ok(cmd!(sh, "{lib_cmd}" ).run()?)

}
