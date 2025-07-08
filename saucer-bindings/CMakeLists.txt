cmake_minimum_required(VERSION 3.21)
project(saucer-bindings LANGUAGES CXX VERSION 5.0.0)

# --------------------------------------------------------------------------------------------------------
# Library switches
# --------------------------------------------------------------------------------------------------------

option(saucer_desktop "Enable support for the desktop module" ON)
option(saucer_pdf "Enable support for the pdf module" ON)
option(SAUCERS_SHARED_LIB "Build a shared library" ON)

# --------------------------------------------------------------------------------------------------------
# CMake options
# --------------------------------------------------------------------------------------------------------

set(CMAKE_POSITION_INDEPENDENT_CODE ON)

# --------------------------------------------------------------------------------------------------------
# Setup library
# --------------------------------------------------------------------------------------------------------

if (SAUCERS_SHARED_LIB)
    add_library(${PROJECT_NAME} SHARED)
else ()
    add_library(${PROJECT_NAME} STATIC)
endif ()

add_library(saucer::bindings ALIAS ${PROJECT_NAME})

target_compile_features(${PROJECT_NAME} PRIVATE cxx_std_23)
set_target_properties(${PROJECT_NAME} PROPERTIES CXX_STANDARD 23 CXX_EXTENSIONS OFF CXX_STANDARD_REQUIRED ON)

# if (PROJECT_IS_TOP_LEVEL AND NOT MSVC AND NOT CMAKE_CXX_SIMULATE_ID MATCHES "MSVC")
#     target_compile_options(${PROJECT_NAME} PRIVATE -Wall -Wextra -Wpedantic -Werror -pedantic -pedantic-errors -Wfatal-errors)
# endif ()

if (NOT MSVC)
    target_compile_options(${PROJECT_NAME} PRIVATE -Wno-unknown-warning-option -Wno-missing-field-initializers -Wno-cast-function-type)
endif ()

# --------------------------------------------------------------------------------------------------------
# Export header
# --------------------------------------------------------------------------------------------------------

include("cmake/hide.cmake")
include("cmake/export.cmake")

saucer_bindings_hide_symbols(${PROJECT_NAME})
saucer_bindings_export(${PROJECT_NAME} "SAUCER_EXPORT")

# --------------------------------------------------------------------------------------------------------
# Include directories
# --------------------------------------------------------------------------------------------------------

target_include_directories(${PROJECT_NAME} PUBLIC "include")
target_include_directories(${PROJECT_NAME} PRIVATE "include/saucer" "private")

# --------------------------------------------------------------------------------------------------------
# Add Sources
# --------------------------------------------------------------------------------------------------------

target_sources(${PROJECT_NAME} PRIVATE
        "src/app.cpp"
        "src/options.cpp"

        "src/memory.cpp"

        "src/icon.cpp"
        "src/stash.cpp"
        "src/script.cpp"
        "src/scheme.cpp"

        "src/navigation.cpp"
        "src/preferences.cpp"

        "src/window.cpp"
        "src/webview.cpp"
)

# --------------------------------------------------------------------------------------------------------
# Setup Dependencies
# --------------------------------------------------------------------------------------------------------

include("cmake/cpm.cmake")

CPMFindPackage(
        NAME saucer
        VERSION 6.0.0
        GIT_REPOSITORY "https://github.com/saucer/saucer"
        OPTIONS "saucer_static ON" "CMAKE_WINDOWS_EXPORT_ALL_SYMBOLS ON" "saucer_package_all ON"
)

# Patches saucer sources
set(SAUCER_PATCHED_FLAG "${CMAKE_CURRENT_BINARY_DIR}/.saucer_patched")

find_program(PATCH_COMMAND patch)
if (NOT PATCH_COMMAND)
    message(FATAL_ERROR "Patch command not found. Please install 'patch' or ensure it's in your PATH.")
endif ()

add_custom_command(
        OUTPUT "${SAUCER_PATCHED_FLAG}"
        COMMAND ${CMAKE_COMMAND} -E touch "${SAUCER_PATCHED_FLAG}"
        COMMAND "${PATCH_COMMAND}" -p0 -i "${CMAKE_CURRENT_SOURCE_DIR}/patches/cocoa.app.mm.patch"
        COMMAND "${PATCH_COMMAND}" -p0 -i "${CMAKE_CURRENT_SOURCE_DIR}/patches/wkg.scheme.cpp.patch"
        WORKING_DIRECTORY "${saucer_SOURCE_DIR}/src"
        COMMENT "Patching saucer sources..."
)

add_custom_target(
        patch_saucer
        ALL
        DEPENDS
        "${SAUCER_PATCHED_FLAG}"
)

add_dependencies(${PROJECT_NAME} patch_saucer)

target_link_libraries(${PROJECT_NAME} PUBLIC saucer::saucer)

# --------------------------------------------------------------------------------------------------------
# Setup Modules
# --------------------------------------------------------------------------------------------------------

include("cmake/module.cmake")

if (saucer_desktop)
    add_subdirectory("modules/desktop")
endif ()

if (saucer_pdf)
    add_subdirectory("modules/pdf")
endif ()

install(
        TARGETS ${PROJECT_NAME}
        ARCHIVE DESTINATION ${CMAKE_INSTALL_LIBDIR}
        LIBRARY DESTINATION ${CMAKE_INSTALL_LIBDIR}
        RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR}
)