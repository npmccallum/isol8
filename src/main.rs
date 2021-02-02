#![deny(clippy::all)]
#![allow(dead_code)]

mod netlink;

use netlink::{Address, Interface};

use std::ffi::CString;
use std::io::ErrorKind;
use std::net::IpAddr;
use std::os::unix::io::AsRawFd;
use std::str::FromStr;

fn find(addr: IpAddr) -> Result<Address, ErrorKind> {
    for address in Address::list().unwrap() {
        if address.subnet().contains(addr) && address.address() != addr {
            return Ok(address);
        }
    }

    Err(ErrorKind::InvalidData)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let addr = IpAddr::from_str(&args.next().unwrap()).unwrap();
    let argv: Vec<CString> = args.map(|x| CString::new(x).unwrap()).collect();

    // Save the original namespace
    let curns = std::fs::File::open("/proc/self/ns/net").unwrap();

    // Create the new namespace
    nix::sched::unshare(nix::sched::CloneFlags::CLONE_NEWNET).unwrap();
    let newns = std::fs::File::open("/proc/self/ns/net").unwrap();

    // Swap back the original namespace
    nix::sched::setns(curns.as_raw_fd(), nix::sched::CloneFlags::CLONE_NEWNET).unwrap();

    // Create our macvlan interface in the new namespace
    let address = find(addr).expect("unable to find matching subnet");
    let mut iface = address.interface().expect("unable to find matching iface");
    let ipvlan = iface.add_ipvlan("ipvl0").expect("error creating ipvlan");
    ipvlan.move_to_namespace(newns.as_raw_fd()).unwrap();

    // Swap to the new namespace and destroy the old one
    nix::sched::setns(newns.as_raw_fd(), nix::sched::CloneFlags::CLONE_NEWNET).unwrap();
    drop(curns);
    drop(newns);

    let mut ipvlan = Interface::find("ipvl0").expect("unable to find ipvlan");
    ipvlan
        .add_address(addr, address.subnet().prefix())
        .expect("unable to add address");
    ipvlan.up().expect("unable to bring up ipvlan");
    ipvlan
        .add_gateway(address.address())
        .expect("unable to add gatweay to ipvlan");

    /*nix::sched::unshare(nix::sched::CloneFlags::CLONE_NEWNS).unwrap();
    nix::unistd::chdir("/").unwrap();

    let mounts = std::fs::File::open("/proc/mounts").unwrap();
    let mounts = BufReader::new(mounts);

    let mut mounts: Vec<CString> = mounts.lines().map(|line| CString::new(line.unwrap().split(" ").nth(1).unwrap()).unwrap()).collect();
    mounts.sort();
    mounts.reverse();

    for entry in std::fs::read_dir("/proc/self/fd").unwrap() {
        let entry = entry.unwrap();
        eprintln!("{:?}: {:?}", entry.path(), nix::fcntl::readlink(&entry.path()).unwrap());
    }

    for mount in &mounts[1..] {
        eprintln!("{:?}", mount);
        match nix::mount::umount(mount.as_ref()) {
            Ok(..) => continue,
            Err(..) => continue,
        }
    }*/

    //nix::unistd::system("dhclient").unwrap();

    nix::unistd::execvp(&argv[0], &argv).unwrap();
}
