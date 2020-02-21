#!/usr/bin/env bash

cargo clippy -- \
  --deny warnings \
  --allow clippy::new_without_default \
  --allow clippy::unneeded-field-pattern \
  --allow clippy::unit_arg
