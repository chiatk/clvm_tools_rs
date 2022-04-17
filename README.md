clvm_tools_rs
=

This is a second-hand port of chia's [clvm tools](https://github.com/Chia-Network/clvm_tools/) to rust via the work of
ChiaMineJP porting to typescript.  This would have been a lot harder to
get to where it is without prior work mapping out the types of various
semi-dynamic things (thanks, ChiaMineJP).

Some reasons for doing this are:

 - Chia switched the clvm implementation to rust: [clvm_rs](https://github.com/Chia-Network/clvm_rs), and this code may both pick up speed and track clvm better being in the same language.
 
 - I wrote a new compiler with a simpler, less intricate structure that should be easier to improve and verify in the future in ocaml: [ochialisp](https://github.com/prozacchiwawa/ochialisp).

 - Also it's faster even in this unoptimized form.

All acceptance tests i've brought over so far work, and more are being added.
As of now, I'm not aware of anything that shouldn't be authentic when running
these command line tools from clvm_tools in their equivalents in this repository

 - opc
 
 - opd
 
 - run
 
 - brun
 
argparse was ported to javascript and I believe I have faithfully reproduced it
as it is used in cmds, so command line parsing should work similarly in all three
versions.

The directory structure is expected to be:

    src/classic  <-- any ported code with heritage pointing back to
                     the original chia repo.
                    
    src/compiler <-- a newer compiler (ochialisp) with a simpler
                     structure.  Select new style compilation by
                     including a `(include *standard-cl-21*)`
                     form in your toplevel `mod` form.

Use with chia-blockchain
===

    # Activate your venv, then
    $ maturin develop --release




Linux Build

https://tecadmin.net/install-python-3-8-ubuntu/

sudo apt-get install build-essential checkinstall
sudo apt-get install libreadline-gplv2-dev libncursesw5-dev libssl-dev \
    libsqlite3-dev tk-dev libgdbm-dev libc6-dev libbz2-dev libffi-dev zlib1g-dev

cd /opt
sudo wget https://www.python.org/ftp/python/3.8.12/Python-3.8.12.tgz
sudo tar xzf Python-3.8.12.tgz

cd Python-3.8.12
sudo ./configure --enable-optimizations
sudo make altinstall