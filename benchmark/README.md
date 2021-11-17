# Benchmarking

## Setup
- 16 core - AMD Ryzen 5950x
- Fedora 35 - Linux 5.14.16-301.fc35.x86_64
- `sudo dnf install kernel-tools`

## Criterion Benchmark
```shell
sudo cpupower frequency-set --governor performance
sudo sh -c "echo 0 > /sys/devices/system/cpu/cpufreq/boost"
sudo sh -c "echo off > /sys/devices/system/cpu/smt/control"

# Make sure there's FREE memory
# Don't do anything else on the system
taskset --cpu-list 15 cargo bench --features enable_criterion -- --warm-up-time 1 --output-format bencher

sudo sh -c "echo on > /sys/devices/system/cpu/smt/control"
sudo sh -c "echo 1 > /sys/devices/system/cpu/cpufreq/boost"
sudo cpupower frequency-set --governor schedutil
```

https://bheisler.github.io/criterion.rs/book/user_guide/command_line_options.html#baselines

```shell
# git checkout or local history
taskset --cpu-list 15 cargo bench --features enable_criterion -- --warm-up-time 1 --save-baseline VERSION1
# git checkout or local history
taskset --cpu-list 15 cargo bench --features enable_criterion -- --warm-up-time 1 --save-baseline VERSION2

# compare:
cargo bench --features enable_criterion -- --verbose --load-baseline VERSION1 --baseline VERSION2
```

## Perf stat Benchmark
```shell
sudo cpupower frequency-set --governor performance
sudo sh -c "echo 0 > /sys/devices/system/cpu/cpufreq/boost"
sudo sh -c "echo off > /sys/devices/system/cpu/smt/control"
sudo sh -c "echo 0 > /proc/sys/kernel/nmi_watchdog"

# Make sure there's FREE memory
# Don't do anything else on the system
perf stat ...

sudo sh -c "echo 1 > /proc/sys/kernel/nmi_watchdog"
sudo sh -c "echo on > /sys/devices/system/cpu/smt/control"
sudo sh -c "echo 1 > /sys/devices/system/cpu/cpufreq/boost"
sudo cpupower frequency-set --governor schedutil
```

## iai Benchmark
```shell
cargo bench --features enable_iai
```

## Debugging
```shell
lscpu
cat /sys/devices/system/cpu/smt/active
```

## TODO: Prioritizing process
problem: I installed cargo as a user...

`sudo nice -n -10 taskset --cpu-list 15 /home/dennis/.cargo/bin/cargo bench`

sudo -E doesn't help...

## Resources
- https://man7.org/linux/man-pages/man1/perf-stat.1.html
- https://github.com/bheisler/criterion.rs
- https://github.com/bheisler/iai


- https://easyperf.net/blog/2019/08/02/Perf-measurement-environment-on-Linux
- https://wiki.archlinux.org/title/CPU_frequency_scaling
- https://serverfault.com/a/967597
- https://sled.rs/perf
