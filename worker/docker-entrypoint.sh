#!/bin/sh
# Substitute the AUTH_TOKEN placeholder in workerd.capnp at container start.
# If AUTH_TOKEN env var is not set, the placeholder is replaced with an empty
# string — the worker will then allow all tunnel registrations (no auth).
sed "s/__AUTH_TOKEN__/${AUTH_TOKEN:-}/g" /worker/worker.capnp > /tmp/worker-run.capnp
exec /workerd serve /tmp/worker-run.capnp --verbose
