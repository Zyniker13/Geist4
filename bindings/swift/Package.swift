// swift-tools-version:5.9
import PackageDescription

// The Swift binding over the geist4 C ABI. CGeist4 is the hand-assembled
// XCFramework the make-xcframework.sh script builds from geist4-ffi's
// static library and the generated header; it is never committed.
//
// The contract suite is a plain executable (Geist4Contract), not an
// XCTest target: the Command Line Tools alone do not ship XCTest.framework,
// and the C and Node contract suites are plain executables too.
let package = Package(
    name: "Geist4",
    platforms: [.macOS(.v12), .iOS(.v13)],
    products: [
        .library(name: "Geist4", targets: ["Geist4"])
    ],
    targets: [
        .binaryTarget(name: "CGeist4", path: "CGeist4.xcframework"),
        .target(name: "Geist4", dependencies: ["CGeist4"]),
        .executableTarget(name: "Geist4Contract", dependencies: ["Geist4"]),
        .executableTarget(name: "Geist4Cross", dependencies: ["Geist4"]),
        .executableTarget(name: "Geist4HostNotes", dependencies: ["Geist4"]),
    ]
)
