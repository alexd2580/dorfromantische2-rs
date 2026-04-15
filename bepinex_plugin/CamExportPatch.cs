// Injected into Assembly-CSharp.dll via Mono.Cecil.
// Called from CameraMovement.LateUpdate() every frame.
using UnityEngine;
using System.IO;

public static class CamExportPatch
{
    private static int frameCounter = 0;
    private static readonly string exportPath = "camera_pos.txt";
    private static readonly string importPath = "camera_set.txt";
    private static readonly string debugLog = "camera_debug.txt";

    private static void Log(string msg)
    {
        try
        {
            File.AppendAllText(debugLog, msg + "\n");
        }
        catch { }
    }

    public static void Export()
    {
        if (++frameCounter % 10 != 0) return;

        // One-time debug: log paths and working directory
        if (frameCounter == 10)
        {
            Log("=== CamExportPatch starting ===");
            Log("exportPath: " + exportPath);
            Log("importPath: " + importPath);
            Log("Directory.GetCurrentDirectory(): " + Directory.GetCurrentDirectory());
            Log("System.AppDomain.CurrentDomain.BaseDirectory: " + System.AppDomain.CurrentDomain.BaseDirectory);
            try
            {
                var files = Directory.GetFiles(".");
                Log("Files in CWD (" + files.Length + "):");
                foreach (var f in files)
                    Log("  " + f);
            }
            catch (System.Exception e) { Log("GetFiles failed: " + e.Message); }
        }

        // Export camera position
        var cam = Camera.main;
        if (cam == null)
        {
            if (frameCounter <= 30) Log("Camera.main is null at frame " + frameCounter);
            return;
        }

        var pos = cam.transform.position;
        var rot = cam.transform.eulerAngles;
        var fov = cam.fieldOfView;

        try
        {
            File.WriteAllText(exportPath, string.Format(
                "{0:F4} {1:F4} {2:F4} {3:F4} {4:F4} {5:F4} {6:F4}",
                pos.x, pos.y, pos.z, rot.x, rot.y, rot.z, fov));
        }
        catch (System.Exception e)
        {
            Log("EXPORT FAILED: " + e.GetType().Name + ": " + e.Message);
        }

        // Import: try to read camera_set.txt, move camera, delete the file
        try
        {
            var line = File.ReadAllText(importPath).Trim();
            Log("READ camera_set.txt: '" + line + "'");
            File.Delete(importPath);
            Log("DELETED camera_set.txt");

            var parts = line.Split(' ');
            Log("SPLIT into " + parts.Length + " parts");
            if (parts.Length >= 3)
            {
                float x = float.Parse(parts[0]);
                float z = float.Parse(parts[2]);
                Log("PARSED x=" + x + " z=" + z);

                var camParent = cam.transform.parent;
                Log("cam.transform.parent: " + (camParent != null ? camParent.name : "NULL"));
                if (camParent != null)
                {
                    var camGrandParent = camParent.parent;
                    Log("cam.transform.parent.parent: " + (camGrandParent != null ? camGrandParent.name : "NULL"));
                    if (camGrandParent != null)
                    {
                        Log("BEFORE camGrandParent.position: " + camGrandParent.position);
                        camGrandParent.position = new Vector3(x, 0, z);
                        Log("AFTER camGrandParent.position: " + camGrandParent.position);
                    }
                }
            }
        }
        catch (FileNotFoundException)
        {
            // Expected — file doesn't exist most of the time
        }
        catch (System.Exception e)
        {
            Log("IMPORT ERROR: " + e.GetType().Name + ": " + e.Message + "\n" + e.StackTrace);
        }
    }
}
