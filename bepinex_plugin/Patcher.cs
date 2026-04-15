// Patches Assembly-CSharp.dll to inject camera position export + smooth import.
// Uses ONLY the game's own type references (no .NET 8 types).
// Generates all IL inline — no external DLL needed.
//
// Export: writes CameraParent position, rotation, FOV, and anchor Z to camera_pos.txt
// Import: reads camera_set.txt and calls MoveCameraTowardsPrecisePosition for smooth movement
//
// Usage: dotnet run --project Patcher.csproj [path-to-Managed-folder]

using Mono.Cecil;
using Mono.Cecil.Cil;
using System;
using System.IO;
using System.Linq;

class Patcher
{
    static void Main(string[] args)
    {
        var managedDir = args.Length > 0 ? args[0] :
            Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
                ".local/share/Steam/steamapps/common/Dorfromantik/Dorfromantik_Data/Managed"
            );

        var asmPath = Path.Combine(managedDir, "Assembly-CSharp.dll");
        var backupPath = asmPath + ".orig";
        if (!File.Exists(asmPath))
        {
            Console.Error.WriteLine($"Not found: {asmPath}");
            return;
        }

        if (!File.Exists(backupPath))
        {
            File.Copy(asmPath, backupPath);
            Console.WriteLine($"Backed up: {backupPath}");
        }

        Console.WriteLine($"Patching: {asmPath}");

        var resolver = new DefaultAssemblyResolver();
        resolver.AddSearchDirectory(managedDir);

        using var asm = AssemblyDefinition.ReadAssembly(asmPath,
            new ReaderParameters { AssemblyResolver = resolver, ReadWrite = true });

        var module = asm.MainModule;

        if (module.Types.Any(t => t.Name == "CamExportPatch"))
        {
            Console.WriteLine("Already patched! To re-patch, restore from .orig first.");
            return;
        }

        // Primitives from module.TypeSystem (references mscorlib, not .NET 8)
        var voidType = module.TypeSystem.Void;
        var intType = module.TypeSystem.Int32;
        var stringType = module.TypeSystem.String;
        var objectType = module.TypeSystem.Object;
        var floatType = module.TypeSystem.Single;

        // Game assemblies
        var unityCore = resolver.Resolve(new AssemblyNameReference("UnityEngine.CoreModule", new Version()));
        var mscorlib = resolver.Resolve(new AssemblyNameReference("mscorlib", new Version()));

        // Types
        var cameraType = unityCore.MainModule.Types.First(t => t.FullName == "UnityEngine.Camera");
        var transformType = unityCore.MainModule.Types.First(t => t.FullName == "UnityEngine.Transform");
        var vector3Type = unityCore.MainModule.Types.First(t => t.FullName == "UnityEngine.Vector3");
        var componentType = unityCore.MainModule.Types.First(t => t.FullName == "UnityEngine.Component");
        var cameraMovementType = module.Types.First(t => t.Name == "CameraMovement");

        // Methods/properties
        var getMain = module.ImportReference(cameraType.Properties.First(p => p.Name == "main").GetMethod);
        var getTransform = module.ImportReference(componentType.Properties.First(p => p.Name == "transform").GetMethod);
        var getPosition = module.ImportReference(transformType.Properties.First(p => p.Name == "position").GetMethod);
        var getLocalPosition = module.ImportReference(transformType.Properties.First(p => p.Name == "localPosition").GetMethod);
        var getEulerAngles = module.ImportReference(transformType.Properties.First(p => p.Name == "eulerAngles").GetMethod);
        var getFieldOfView = module.ImportReference(cameraType.Properties.First(p => p.Name == "fieldOfView").GetMethod);
        var getParent = module.ImportReference(transformType.Properties.First(p => p.Name == "parent").GetMethod);

        // CameraMovement.MoveCameraTowardsPrecisePosition(Vector3, float)
        var moveCamMethod = module.ImportReference(
            cameraMovementType.Methods.First(m => m.Name == "MoveCameraTowardsPrecisePosition"));

        var v3TypeRef = module.ImportReference(vector3Type);
        var v3x = module.ImportReference(vector3Type.Fields.First(f => f.Name == "x"));
        var v3y = module.ImportReference(vector3Type.Fields.First(f => f.Name == "y"));
        var v3z = module.ImportReference(vector3Type.Fields.First(f => f.Name == "z"));

        // File I/O
        var fileType = mscorlib.MainModule.Types.First(t => t.FullName == "System.IO.File");
        var writeAllText = module.ImportReference(
            fileType.Methods.First(m => m.Name == "WriteAllText" && m.Parameters.Count == 2));
        var readAllText = module.ImportReference(
            fileType.Methods.First(m => m.Name == "ReadAllText" && m.Parameters.Count == 1));
        var fileDelete = module.ImportReference(
            fileType.Methods.First(m => m.Name == "Delete" && m.Parameters.Count == 1));

        // String
        var stringFormatArr = module.ImportReference(
            mscorlib.MainModule.Types.First(t => t.FullName == "System.String")
                .Methods.First(m => m.Name == "Format" && m.Parameters.Count == 2
                    && m.Parameters[1].ParameterType.IsArray));
        var stringTrim = module.ImportReference(
            mscorlib.MainModule.Types.First(t => t.FullName == "System.String")
                .Methods.First(m => m.Name == "Trim" && m.Parameters.Count == 0));
        var stringSplit = module.ImportReference(
            mscorlib.MainModule.Types.First(t => t.FullName == "System.String")
                .Methods.First(m => m.Name == "Split" && m.Parameters.Count == 1
                    && m.Parameters[0].ParameterType.IsArray
                    && m.Parameters[0].ParameterType.GetElementType().FullName == "System.Char"));
        var singleParse = module.ImportReference(
            mscorlib.MainModule.Types.First(t => t.FullName == "System.Single")
                .Methods.First(m => m.Name == "Parse" && m.Parameters.Count == 1));

        // Exceptions
        var exceptionType = module.ImportReference(
            mscorlib.MainModule.Types.First(t => t.FullName == "System.Exception"));
        var fileNotFoundType = module.ImportReference(
            mscorlib.MainModule.Types.First(t => t.FullName == "System.IO.FileNotFoundException"));

        // Vector3 ctor
        var v3Ctor = module.ImportReference(
            vector3Type.Methods.First(m => m.IsConstructor && m.Parameters.Count == 3));

        // === Create CamExportPatch type ===
        var patchType = new TypeDefinition("", "CamExportPatch",
            TypeAttributes.Public | TypeAttributes.Abstract | TypeAttributes.Sealed,
            objectType);
        module.Types.Add(patchType);

        var counterField = new FieldDefinition("frameCounter",
            FieldAttributes.Private | FieldAttributes.Static, intType);
        patchType.Fields.Add(counterField);

        // === Export(CameraMovement camMov) — takes instance so we can call MoveCameraTowardsPrecisePosition ===
        var exportMethod = new MethodDefinition("Export",
            MethodAttributes.Public | MethodAttributes.Static, voidType);
        exportMethod.Parameters.Add(new ParameterDefinition("camMov",
            ParameterAttributes.None, module.ImportReference(cameraMovementType)));
        patchType.Methods.Add(exportMethod);
        exportMethod.Body.InitLocals = true;

        // Locals
        var posLocal = new VariableDefinition(v3TypeRef);           // 0: CameraParent position
        var rotLocal = new VariableDefinition(v3TypeRef);           // 1: CameraParent eulerAngles
        var fovLocal = new VariableDefinition(floatType);           // 2: FOV
        var camLocal = new VariableDefinition(module.ImportReference(cameraType)); // 3: Camera.main
        var transformLocal = new VariableDefinition(module.ImportReference(transformType)); // 4: cam.transform
        var contentLocal = new VariableDefinition(stringType);      // 5: formatted string
        var anchorPosLocal = new VariableDefinition(v3TypeRef);     // 6: CameraAnchor localPosition
        var importLineLocal = new VariableDefinition(stringType);   // 7
        var partsLocal = new VariableDefinition(new ArrayType(stringType)); // 8
        var importXLocal = new VariableDefinition(floatType);       // 9
        var importZLocal = new VariableDefinition(floatType);       // 10
        var newPosLocal = new VariableDefinition(v3TypeRef);        // 11

        exportMethod.Body.Variables.Add(posLocal);
        exportMethod.Body.Variables.Add(rotLocal);
        exportMethod.Body.Variables.Add(fovLocal);
        exportMethod.Body.Variables.Add(camLocal);
        exportMethod.Body.Variables.Add(transformLocal);
        exportMethod.Body.Variables.Add(contentLocal);
        exportMethod.Body.Variables.Add(anchorPosLocal);
        exportMethod.Body.Variables.Add(importLineLocal);
        exportMethod.Body.Variables.Add(partsLocal);
        exportMethod.Body.Variables.Add(importXLocal);
        exportMethod.Body.Variables.Add(importZLocal);
        exportMethod.Body.Variables.Add(newPosLocal);

        var il = exportMethod.Body.GetILProcessor();
        var retInstr = il.Create(OpCodes.Ret);

        // frameCounter++; if (frameCounter % 10 != 0) return;
        il.Append(il.Create(OpCodes.Ldsfld, counterField));
        il.Append(il.Create(OpCodes.Ldc_I4_1));
        il.Append(il.Create(OpCodes.Add));
        il.Append(il.Create(OpCodes.Dup));
        il.Append(il.Create(OpCodes.Stsfld, counterField));
        il.Append(il.Create(OpCodes.Ldc_I4, 10));
        il.Append(il.Create(OpCodes.Rem));
        il.Append(il.Create(OpCodes.Brtrue, retInstr));

        // cam = Camera.main; if (cam == null) return;
        il.Append(il.Create(OpCodes.Call, getMain));
        il.Append(il.Create(OpCodes.Stloc_3));
        il.Append(il.Create(OpCodes.Ldloc_3));
        il.Append(il.Create(OpCodes.Brfalse, retInstr));

        // transform = cam.transform
        il.Append(il.Create(OpCodes.Ldloc_3));
        il.Append(il.Create(OpCodes.Callvirt, getTransform));
        il.Append(il.Create(OpCodes.Stloc, transformLocal));

        // pos = cam.transform.parent.parent.position (CameraParent)
        il.Append(il.Create(OpCodes.Ldloc, transformLocal));
        il.Append(il.Create(OpCodes.Callvirt, getParent));   // CameraAnchor
        il.Append(il.Create(OpCodes.Callvirt, getParent));   // CameraParent
        il.Append(il.Create(OpCodes.Callvirt, getPosition));
        il.Append(il.Create(OpCodes.Stloc_0));

        // rot = CameraParent.eulerAngles
        il.Append(il.Create(OpCodes.Ldloc, transformLocal));
        il.Append(il.Create(OpCodes.Callvirt, getParent));
        il.Append(il.Create(OpCodes.Callvirt, getParent));
        il.Append(il.Create(OpCodes.Callvirt, getEulerAngles));
        il.Append(il.Create(OpCodes.Stloc_1));

        // fov = cam.fieldOfView
        il.Append(il.Create(OpCodes.Ldloc_3));
        il.Append(il.Create(OpCodes.Callvirt, getFieldOfView));
        il.Append(il.Create(OpCodes.Stloc_2));

        // anchorPos = cam.transform.parent.localPosition (CameraAnchor)
        il.Append(il.Create(OpCodes.Ldloc, transformLocal));
        il.Append(il.Create(OpCodes.Callvirt, getParent));
        il.Append(il.Create(OpCodes.Callvirt, getLocalPosition));
        il.Append(il.Create(OpCodes.Stloc, anchorPosLocal));

        // Format: "{0:F4} {1:F4} {2:F4} {3:F4} {4:F4} {5:F4} {6:F4} {7:F4}"
        // Values: pos.x pos.y pos.z rot.x rot.y rot.z fov anchorPos.z
        il.Append(il.Create(OpCodes.Ldstr, "{0:F4} {1:F4} {2:F4} {3:F4} {4:F4} {5:F4} {6:F4} {7:F4}"));
        il.Append(il.Create(OpCodes.Ldc_I4_8));
        il.Append(il.Create(OpCodes.Newarr, objectType));

        void EmitArrayStore(int index, VariableDefinition vecLocal, FieldReference field)
        {
            il.Append(il.Create(OpCodes.Dup));
            il.Append(il.Create(OpCodes.Ldc_I4, index));
            il.Append(il.Create(OpCodes.Ldloca_S, vecLocal));
            il.Append(il.Create(OpCodes.Ldfld, field));
            il.Append(il.Create(OpCodes.Box, floatType));
            il.Append(il.Create(OpCodes.Stelem_Ref));
        }

        EmitArrayStore(0, posLocal, v3x);   // pos.x
        EmitArrayStore(1, posLocal, v3y);   // pos.y
        EmitArrayStore(2, posLocal, v3z);   // pos.z
        EmitArrayStore(3, rotLocal, v3x);   // rot.x
        EmitArrayStore(4, rotLocal, v3y);   // rot.y
        EmitArrayStore(5, rotLocal, v3z);   // rot.z

        // array[6] = fov
        il.Append(il.Create(OpCodes.Dup));
        il.Append(il.Create(OpCodes.Ldc_I4_6));
        il.Append(il.Create(OpCodes.Ldloc_2));
        il.Append(il.Create(OpCodes.Box, floatType));
        il.Append(il.Create(OpCodes.Stelem_Ref));

        // array[7] = anchorPos.z (zoom distance, negative)
        EmitArrayStore(7, anchorPosLocal, v3z);

        il.Append(il.Create(OpCodes.Call, stringFormatArr));
        il.Append(il.Create(OpCodes.Stloc, contentLocal));

        // === EXPORT try/catch ===
        var tryStart = il.Create(OpCodes.Ldstr, "camera_pos.txt");
        il.Append(tryStart);
        il.Append(il.Create(OpCodes.Ldloc, contentLocal));
        il.Append(il.Create(OpCodes.Call, writeAllText));
        var leaveTarget = il.Create(OpCodes.Nop);
        il.Append(il.Create(OpCodes.Leave_S, leaveTarget));

        var catchStart = il.Create(OpCodes.Pop);
        il.Append(catchStart);
        il.Append(il.Create(OpCodes.Leave_S, leaveTarget));

        il.Append(leaveTarget);

        exportMethod.Body.ExceptionHandlers.Add(new ExceptionHandler(ExceptionHandlerType.Catch)
        {
            TryStart = tryStart,
            TryEnd = catchStart,
            HandlerStart = catchStart,
            HandlerEnd = leaveTarget,
            CatchType = exceptionType,
        });

        // === IMPORT: read camera_set.txt, call MoveCameraTowardsPrecisePosition ===
        var importLeave = il.Create(OpCodes.Nop);

        var importTryStart = il.Create(OpCodes.Ldstr, "camera_set.txt");
        il.Append(importTryStart);
        il.Append(il.Create(OpCodes.Call, readAllText));
        il.Append(il.Create(OpCodes.Callvirt, stringTrim));
        il.Append(il.Create(OpCodes.Stloc, importLineLocal));

        il.Append(il.Create(OpCodes.Ldstr, "camera_set.txt"));
        il.Append(il.Create(OpCodes.Call, fileDelete));

        // parts = line.Split(new char[]{' '});
        il.Append(il.Create(OpCodes.Ldloc, importLineLocal));
        il.Append(il.Create(OpCodes.Ldc_I4_1));
        il.Append(il.Create(OpCodes.Newarr, module.TypeSystem.Char));
        il.Append(il.Create(OpCodes.Dup));
        il.Append(il.Create(OpCodes.Ldc_I4_0));
        il.Append(il.Create(OpCodes.Ldc_I4, (int)' '));
        il.Append(il.Create(OpCodes.Stelem_I2));
        il.Append(il.Create(OpCodes.Callvirt, stringSplit));
        il.Append(il.Create(OpCodes.Stloc, partsLocal));

        // if (parts.Length < 3) leave;
        il.Append(il.Create(OpCodes.Ldloc, partsLocal));
        il.Append(il.Create(OpCodes.Ldlen));
        il.Append(il.Create(OpCodes.Ldc_I4_3));
        il.Append(il.Create(OpCodes.Blt, importLeave));

        // x = float.Parse(parts[0]); z = float.Parse(parts[2]);
        il.Append(il.Create(OpCodes.Ldloc, partsLocal));
        il.Append(il.Create(OpCodes.Ldc_I4_0));
        il.Append(il.Create(OpCodes.Ldelem_Ref));
        il.Append(il.Create(OpCodes.Call, singleParse));
        il.Append(il.Create(OpCodes.Stloc, importXLocal));

        il.Append(il.Create(OpCodes.Ldloc, partsLocal));
        il.Append(il.Create(OpCodes.Ldc_I4_2));
        il.Append(il.Create(OpCodes.Ldelem_Ref));
        il.Append(il.Create(OpCodes.Call, singleParse));
        il.Append(il.Create(OpCodes.Stloc, importZLocal));

        // Construct target Vector3(x, 0, z)
        il.Append(il.Create(OpCodes.Ldloca_S, newPosLocal));
        il.Append(il.Create(OpCodes.Ldloc, importXLocal));
        il.Append(il.Create(OpCodes.Ldc_R4, 0.0f));
        il.Append(il.Create(OpCodes.Ldloc, importZLocal));
        il.Append(il.Create(OpCodes.Call, v3Ctor));

        // Call camMov.MoveCameraTowardsPrecisePosition(target, 1.0f)
        il.Append(il.Create(OpCodes.Ldarg_0)); // camMov parameter
        il.Append(il.Create(OpCodes.Ldloc, newPosLocal));
        il.Append(il.Create(OpCodes.Ldc_R4, 1.0f)); // maxDuration = 1 second
        il.Append(il.Create(OpCodes.Callvirt, moveCamMethod));

        il.Append(il.Create(OpCodes.Leave_S, importLeave));

        // catch (FileNotFoundException) { }
        var importCatchFnf = il.Create(OpCodes.Pop);
        il.Append(importCatchFnf);
        il.Append(il.Create(OpCodes.Leave_S, importLeave));

        // catch (Exception) { }
        var importCatchAll = il.Create(OpCodes.Pop);
        il.Append(importCatchAll);
        il.Append(il.Create(OpCodes.Leave_S, importLeave));

        il.Append(importLeave);
        il.Append(retInstr);

        exportMethod.Body.ExceptionHandlers.Add(new ExceptionHandler(ExceptionHandlerType.Catch)
        {
            TryStart = importTryStart,
            TryEnd = importCatchFnf,
            HandlerStart = importCatchFnf,
            HandlerEnd = importCatchAll,
            CatchType = fileNotFoundType,
        });
        exportMethod.Body.ExceptionHandlers.Add(new ExceptionHandler(ExceptionHandlerType.Catch)
        {
            TryStart = importTryStart,
            TryEnd = importCatchFnf,
            HandlerStart = importCatchAll,
            HandlerEnd = importLeave,
            CatchType = exceptionType,
        });

        // === Inject into CameraMovement.LateUpdate ===
        // call CamExportPatch::Export(this)  — pass CameraMovement instance
        var lateUpdate = cameraMovementType.Methods.FirstOrDefault(m => m.Name == "LateUpdate");
        if (lateUpdate == null) { Console.Error.WriteLine("LateUpdate not found!"); return; }

        var luIl = lateUpdate.Body.GetILProcessor();
        var first = lateUpdate.Body.Instructions[0];
        luIl.InsertBefore(first, luIl.Create(OpCodes.Ldarg_0)); // push 'this' (CameraMovement)
        luIl.InsertBefore(first, luIl.Create(OpCodes.Call, exportMethod));

        asm.Write();
        Console.WriteLine("Patched successfully!");
        Console.WriteLine("Format: x y z rotX rotY rotZ fov anchorZ");
    }
}
