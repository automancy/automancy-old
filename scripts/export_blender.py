import bpy
import sys

def main():
    dst = sys.argv[-1]

    bpy.context.active_object.select_set(False)

    for obj in filter(lambda o: o.type == 'MESH', bpy.data.objects):
        obj.rotation_euler.z += -3.14159265358979323846264338327950288

        if not obj.data.color_attributes and obj.active_material:
            obj.data.color_attributes.new(name='Col', type='BYTE_COLOR', domain='CORNER')

            color = obj.active_material.diffuse_color

            for datum in obj.data.attributes.active_color.data:
                datum.color = color

    bpy.ops.export_scene.gltf(filepath=dst, check_existing=False, export_format='GLB',
                              export_image_format='NONE', export_texcoords=False, export_materials='NONE',
                              export_apply=True, export_skins=False, export_lights=False, export_yup=False,
                              will_save_settings=False, export_draco_mesh_compression_enable=False)


if __name__ == "__main__":
    main()
