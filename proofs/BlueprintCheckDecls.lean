import Lake.CLI.Main

open Lake Lean

private def usageMessage : String :=
  "usage: lake exe blueprintCheckDecls <path-to-blueprint/lean_decls>"

/--
Check every declaration emitted by Lean Blueprint against all Lean libraries in
this workspace. An empty declaration list is valid: Unit 1 intentionally has
only planned and assumption Blueprint nodes.
-/
unsafe def main (args : List String) : IO UInt32 := do
  unless args.length == 1 do
    IO.eprintln usageMessage
    return 2

  let declarationFile : System.FilePath := args[0]!
  unless ← declarationFile.pathExists do
    IO.eprintln s!"declaration list does not exist: {declarationFile}"
    return 2

  let (elanInstall?, leanInstall?, lakeInstall?) ← findInstall?
  let config ← MonadError.runEIO <|
    mkLoadConfig { elanInstall?, leanInstall?, lakeInstall? }
  let (workspace?, log) ← (loadWorkspace config).run?
  log.replay (logger := .stderr)
  let some workspace := workspace? | return 2

  let imports := workspace.root.leanLibs.flatMap fun library =>
    library.config.roots.map fun moduleName => { module := moduleName }
  enableInitializersExecution
  let environment ← Lean.importModules imports {}

  let mut ok := true
  for rawLine in ← IO.FS.lines declarationFile do
    let declaration := rawLine.trimAscii.toString
    if declaration.isEmpty then
      continue
    unless environment.contains declaration.toName do
      IO.eprintln s!"missing Lean declaration: {declaration}"
      ok := false

  return if ok then 0 else 1
