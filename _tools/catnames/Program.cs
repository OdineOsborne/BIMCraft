// Reads the published BuiltInCategory constants of an installed Revit API assembly:
// names and numbers only, through metadata, never loading or running Autodesk's code.
using System;
using System.Collections.Generic;
using System.IO;
using System.Reflection;

if (args.Length < 1) { Console.Error.WriteLine("usage: catnames <RevitAPI.dll> [runtime dir]"); return 2; }
var api = Path.GetFullPath(args[0]);
var runtime = args.Length > 1 ? args[1] : Path.GetDirectoryName(typeof(object).Assembly.Location);
var files = new List<string> { api };
files.AddRange(Directory.GetFiles(runtime, "*.dll"));
using var mlc = new MetadataLoadContext(new PathAssemblyResolver(files));
var asm = mlc.LoadFromAssemblyPath(api);
var type = asm.GetType("Autodesk.Revit.DB.BuiltInCategory");
if (type is null) { Console.Error.WriteLine("BuiltInCategory not found"); return 1; }
var rows = 0;
foreach (var f in type.GetFields(BindingFlags.Public | BindingFlags.Static))
{
    var v = f.GetRawConstantValue();
    if (v is null) continue;
    Console.WriteLine($"{Convert.ToInt64(v)}\t{f.Name}");
    rows++;
}
Console.Error.WriteLine($"{rows} values");
return 0;
