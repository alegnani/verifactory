# Factorio Verify
![](teaser.png)

Factorio Verify is a verifier for Factorio blueprints, enabling you to automatically check logical properties on a blueprint.
Currently this is limited to checking whether or not a belt-balancer works and if the inputs get all drained the same, leading to no imbalances.

## How it works
 - A blueprint is loaded with **File > Open blueprint** or **File > Open blueprint file**.
 - Automatically added inputs/outputs should be removed accordingly (Every belt or splitter ending or starting from nothing gets flagged as input/output).
 - Every entity of the blueprint is then transformed into one or more nodes in a graph (using an intermediate representation).
 - The graph is simplified and the throughputs of the different edges are minimized as much as possible.
 - Using a SAT solver, like z3, the graph is turned into a series of logical formulas and constraints.
 - These constraints can then be used to prove or disprove certain logical properties.

## Performance

Although no optimizations have been made the verifier is extremely performant. This is due to the fact that the verifier does not need to try all the combinations of inputs and simulate the actual blueprint, relying instead on the resolution of logical formulas using a SAT solver.

Take for example the following [64x64 belt balancer](https://fbe.teoxoy.com/?source=https://www.factorio.school/api/blueprintData/322abb92820177a1d15d3d7dea13353bae52a723/position/14). To load and verify both belt-balancing and equal-drain properties it takes less then 1 second! With any other tool this would take considerably longer.


## Features and Todos

 - Standard properties:
   - [x] Belt balancing
   - [x] Equal input drain
   - [x] Throughput unlimited
   - [x] Universal balancer
 - [x] Counter example generation 
 - [ ] Correct colors for the different belts
 - [ ] Find a nice way to visualize or export a counter example
 - [ ] Resizable and movable canvas
 - [ ] Support for dual-lane belts
 - [ ] Resizable and movable canvas
 - [ ] Support for inserters and assemblers
 - [ ] Custom language to express arbitrary properties
 - [ ] DOCS!

## Installation

The latest version of the program can be found [here](https://github.com/alegnani/factorio_verify/releases) for both Windows and Linux. 
This option ships factorio_verify bundled with z3.

### Building from source

`factorio_verify` can be compiled either with `z3` bundled with it or needing a valid installation of it on the system. For Windows it is recommended to go for the bundled version.

#### Building the standalone version

To build: `cargo build --release`. To run: `cargo run --release`.
Executable can be found in `target/release`.

#### Building the bundled version
To build: `cargo build --release --features build_z3`. To run: `cargo run --release --features --build_z3`.
Executable can be found in `target/release`.

## Contributing

> **Warning!**
> The project is still in a very early stage, thus lots of design choices are still open and the code quality is very low

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change. Note that documentation is non-existant and the design document is mostly incomplete.
Bug reports are greatly appreciated. :)

## Design Document

More information about how the project is structured can be found under this [link](design_doc/design_doc.md).
This includes the design document specifying how the underlying conversion from Factorio blueprints to the z3 model works.