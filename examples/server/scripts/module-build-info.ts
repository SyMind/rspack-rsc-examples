import { Module } from "@rspack/core";

const BUILD_INFO_MAP = new WeakMap<Module, Record<string, any>>();

export function getModuleBuildInfo(module: Module): Record<string, any> {
    return BUILD_INFO_MAP.get(module) ?? {};
}

export function setModuleBuildInfo(module: Module, buildInfo: Record<string, any>) {
    BUILD_INFO_MAP.set(module, buildInfo);
}
