# Notes

- visitor pattern *might* work
- visiting custom structs is easy (fractal). Structs know which models to use for their components, and the choice of model is static (per type)
- encoding primitive types is harder, since different structs may use different models for the same primitives.
- perhaps each type (including primitive types) should advertise their own config, which should be passed in from the parent