/*
 * WiFi GigaChad 420 — NDIS 6.80 Intermediate MUX Driver (driver.c)
 *
 * Exposes "WiFi GigaChad 420 Virtual Network" Virtual Miniport at the upper edge
 * and binds as an NDIS 6.80 Protocol Driver at the lower edge.
 */

#include "gigachadmux.h"

static NDIS_HANDLE g_MiniportDriverHandle = NULL;
static NDIS_HANDLE g_ProtocolDriverHandle = NULL;

NTSTATUS
DriverEntry(
    _In_ PDRIVER_OBJECT DriverObject,
    _In_ PUNICODE_STRING RegistryPath
)
{
    NDIS_STATUS status;
    NDIS_MINIPORT_DRIVER_CHARACTERISTICS mpChars;
    NDIS_PROTOCOL_DRIVER_CHARACTERISTICS protChars;

    NdisZeroMemory(&mpChars, sizeof(mpChars));
    mpChars.Header.Type = NDIS_OBJECT_TYPE_MINIPORT_DRIVER_CHARACTERISTICS;
    mpChars.Header.Revision = NDIS_MINIPORT_DRIVER_CHARACTERISTICS_REVISION_3;
    mpChars.Header.Size = NDIS_SIZEOF_MINIPORT_DRIVER_CHARACTERISTICS_REVISION_3;
    mpChars.MajorNdisVersion = 6;
    mpChars.MinorNdisVersion = 80;
    mpChars.Flags = NDIS_INTERMEDIATE_DRIVER;
    mpChars.InitializeHandlerEx = GigaMiniportInitializeEx;
    mpChars.HaltHandlerEx = GigaMiniportHaltEx;
    mpChars.PauseHandler = GigaMiniportPause;
    mpChars.RestartHandler = GigaMiniportRestart;
    mpChars.SendNetBufferListsHandler = GigaMiniportSendNetBufferLists;
    mpChars.ReturnNetBufferListsHandler = GigaMiniportReturnNetBufferLists;
    mpChars.OidRequestHandler = GigaMiniportOidRequest;
    mpChars.CancelOidRequestHandler = GigaMiniportCancelOidRequest;

    status = NdisMRegisterMiniportDriver(
        DriverObject,
        RegistryPath,
        NULL,
        &mpChars,
        &g_MiniportDriverHandle
    );
    if (status != NDIS_STATUS_SUCCESS) {
        return status;
    }

    NdisZeroMemory(&protChars, sizeof(protChars));
    protChars.Header.Type = NDIS_OBJECT_TYPE_PROTOCOL_DRIVER_CHARACTERISTICS;
    protChars.Header.Revision = NDIS_PROTOCOL_DRIVER_CHARACTERISTICS_REVISION_2;
    protChars.Header.Size = NDIS_SIZEOF_PROTOCOL_DRIVER_CHARACTERISTICS_REVISION_2;
    protChars.MajorNdisVersion = 6;
    protChars.MinorNdisVersion = 80;
    protChars.BindAdapterHandlerEx = GigaProtocolBindAdapterEx;
    protChars.UnbindAdapterHandlerEx = GigaProtocolUnbindAdapterEx;

    status = NdisRegisterProtocolDriver(NULL, &protChars, &g_ProtocolDriverHandle);
    if (status != NDIS_STATUS_SUCCESS) {
        NdisMDeregisterMiniportDriver(g_MiniportDriverHandle);
        return status;
    }

    NdisIMAssociateMiniport(g_MiniportDriverHandle, g_ProtocolDriverHandle);
    return NDIS_STATUS_SUCCESS;
}

NDIS_STATUS
GigaMiniportInitializeEx(
    _In_ NDIS_HANDLE MiniportAdapterHandle,
    _In_ NDIS_HANDLE MiniportDriverContext,
    _In_ PNDIS_MINIPORT_INIT_PARAMETERS MiniportInitParameters
)
{
    UNREFERENCED_PARAMETER(MiniportDriverContext);
    UNREFERENCED_PARAMETER(MiniportInitParameters);

    PGIGA_VELAN_ADAPTER adapter = (PGIGA_VELAN_ADAPTER)NdisAllocateMemoryWithTagPriority(
        MiniportAdapterHandle,
        sizeof(GIGA_VELAN_ADAPTER),
        GIGACHAD_MUX_TAG,
        NormalPoolPriority
    );
    if (!adapter) {
        return NDIS_STATUS_RESOURCES;
    }

    NdisZeroMemory(adapter, sizeof(GIGA_VELAN_ADAPTER));
    adapter->MiniportAdapterHandle = MiniportAdapterHandle;
    /* Locally-administered IEEE 802 MAC: 02:47:43:04:20:01 */
    adapter->VirtualMacAddress[0] = 0x02;
    adapter->VirtualMacAddress[1] = 0x47;
    adapter->VirtualMacAddress[2] = 0x43;
    adapter->VirtualMacAddress[3] = 0x04;
    adapter->VirtualMacAddress[4] = 0x20;
    adapter->VirtualMacAddress[5] = 0x01;
    NdisAllocateSpinLock(&adapter->Lock);

    return NDIS_STATUS_SUCCESS;
}

VOID
GigaMiniportHaltEx(
    _In_ NDIS_HANDLE MiniportAdapterContext,
    _In_ NDIS_HALT_ACTION HaltAction
)
{
    UNREFERENCED_PARAMETER(HaltAction);
    PGIGA_VELAN_ADAPTER adapter = (PGIGA_VELAN_ADAPTER)MiniportAdapterContext;
    if (adapter) {
        NdisFreeSpinLock(&adapter->Lock);
        NdisFreeMemory(adapter, sizeof(GIGA_VELAN_ADAPTER), 0);
    }
}

