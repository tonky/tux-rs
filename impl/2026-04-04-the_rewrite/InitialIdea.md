Let's rewrite in Rust the Tuxedo kernel drivers and Tuxedo Control Center from the ground up.
Those are old codebases, and will benefit from rewrite, refactor and unified Rust codebase.

Make hardware models separate and descriptive in code, with explicit capabilities and mapping to linux subsystems for read/write data.

Share data and models where possible.

Use D-Bus for communicating with TUI. Make it a separate binary crate.

Add library to read/write to kernel interfaces. Use it from D-Bus. 

Keep kernel code to a minimum, try unifying interfaces and usage patterns where possible.

Investigate both kernel(projects/tuxedo-drivers-rs) and tcc(projects/tcc-rs) working prototypes that were already built as a proof of concept.

Plan to support all existing models, and new models support should be added as easily as possible.

Consideration: do we even need a separate D-Bus service? Maybe we can just use TUI(with planned alternative GUI implementation) + kernel interface library? Hmm, but then user will have to use 'sudo' constantly, i guess we still need D-Bus?

Make sure we're implementing TUI first, so we'll have to mark some GUI-only features(like webcam preview, TOMTE etc) as deliberately ignored.

Let's come up with short(2-3 pages) overview(OVERVIEW.md), that we can revise and build our detailed plan off.
