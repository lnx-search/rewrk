mod args;


fn main() -> anyhow::Result<()> {
   let args: args::Args = args::Args::parse();


   Ok(())
}
