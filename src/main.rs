#[macro_use]
extern crate rocket;

#[launch]
fn rocket() -> _ {
    my_little_factory_manager::rocket_initialize()
}
