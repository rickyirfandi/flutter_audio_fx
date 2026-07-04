# Flutter Windows FFI-plugin build glue for the Rust core.
#
# If a prebuilt `flutter_audio_fx_core.dll` is present next to this file
# (placed by the CI build-native workflow before publishing), it is bundled
# as-is. Otherwise the Rust crate at ../rust is built from source via cargo.
cmake_minimum_required(VERSION 3.14)

set(PROJECT_NAME "flutter_audio_fx")
project(${PROJECT_NAME} LANGUAGES CXX)

set(crate_dir "${CMAKE_CURRENT_SOURCE_DIR}/../rust")
set(rust_lib  "${crate_dir}/target/release/flutter_audio_fx_core.dll")
set(prebuilt  "${CMAKE_CURRENT_SOURCE_DIR}/flutter_audio_fx_core.dll")

if(EXISTS "${prebuilt}")
  set(bundled "${prebuilt}")
else()
  find_program(CARGO cargo REQUIRED)
  add_custom_command(
    OUTPUT "${rust_lib}"
    COMMAND ${CARGO} build --release --manifest-path "${crate_dir}/Cargo.toml"
    WORKING_DIRECTORY "${crate_dir}"
    COMMENT "Building flutter_audio_fx_core (Rust, release)"
    VERBATIM)
  add_custom_target(flutter_audio_fx_rust ALL DEPENDS "${rust_lib}")
  set(bundled "${rust_lib}")
endif()

# Copied next to the runner .exe so DynamicLibrary.open() can resolve it.
set(flutter_audio_fx_bundled_libraries
  "${bundled}"
  PARENT_SCOPE)
