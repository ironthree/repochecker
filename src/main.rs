//use warp::Filter;

/*
#[tokio::main]
async fn main() {
    let hello = warp::path!("hello" / String)
        .map(|name| format!("Hello, {}!", name));

    let run = warp::serve(hello)
        .run(([127, 0, 0, 1], 8000));

    println!("Serving at http://localhost:8000 ...");

    run.await;
}
*/

fn main() -> Result<(), String> {
    //let _config: Config = get_config()?;
    //let _overrides: Overrides = get_overrides()?;
    //let _admins = get_admins(&_config.fedora.api_url, _config.fedora.timeout)?;
    //let _rawhide_compose = get_rawhide_compose()?;
    //let _rawhide_contents = get_rawhide_contents("x86_64")?;
    //let _rawhide_closure = get_rawhide_repoclosure("x86_64")?;

    //println!("{:#?}", _rawhide_closure);

    Ok(())
}
