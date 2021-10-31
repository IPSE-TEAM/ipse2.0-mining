#!/usr/bin/env bash

set -e

echo "*** Initializing WASM build environment"

if [ -z $CI ] ; then
   rustup update nightly
   rustup update stable
fi

rustup target add wasm32-unknown-unknown --toolchain nightly